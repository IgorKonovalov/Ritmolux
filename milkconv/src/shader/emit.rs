//! The typed WGSL emitter.
//!
//! Walks the parsed tree with a small type system — `f32`, `vecN<f32>`,
//! `matNxN<f32>`, `bool`, `vecN<bool>` — and emits fully-parenthesized WGSL,
//! inserting the conversions HLSL performed implicitly: scalar→vector splat,
//! larger→smaller vector truncation (the legacy fxc behavior the corpus was
//! compiled against), bool→float in arithmetic, and float→bool in conditions.
//! The output is machine-read only, so it optimizes for being *obviously*
//! correct over being pretty.

use std::collections::HashMap;

use super::parse::{self, Expr, Func, Stmt, Unit};
use super::{LOOP_CAP, MAX_LOOP_DEPTH, MAX_OPS, ShaderError, Stage, Translated};

/// The emitter's type lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    F32,
    Vec(u8),
    Mat(u8),
    Bool,
    BVec(u8),
    Void,
    /// A `rot_*` matrix — indexable only, four `vec4` rows whose `.xyz` is the
    /// `float4x3` row MilkDrop presets read.
    Rot,
}

impl Ty {
    fn wgsl(self) -> String {
        match self {
            Ty::F32 => "f32".into(),
            Ty::Vec(n) => format!("vec{n}<f32>"),
            Ty::Mat(n) => format!("mat{n}x{n}<f32>"),
            Ty::Bool => "bool".into(),
            Ty::BVec(n) => format!("vec{n}<bool>"),
            Ty::Void => "void".into(),
            Ty::Rot => "rot".into(),
        }
    }

    fn zero(self) -> String {
        match self {
            Ty::F32 => "0.0".into(),
            Ty::Vec(n) => format!("vec{n}<f32>()"),
            Ty::Mat(n) => format!("mat{n}x{n}<f32>()"),
            Ty::Bool => "false".into(),
            Ty::BVec(n) => format!("vec{n}<bool>()"),
            Ty::Void | Ty::Rot => "0.0".into(),
        }
    }
}

/// An HLSL type name to the lattice. Every scalar is `f32` (module docs).
fn hlsl_ty(name: &str) -> Result<Ty, ShaderError> {
    if name == "void" {
        return Ok(Ty::Void);
    }
    if name == "bool" {
        return Ok(Ty::Bool);
    }
    if matches!(name, "float" | "int" | "uint" | "half" | "double") {
        return Ok(Ty::F32);
    }
    if let Some(n) = parse::is_vector_type(name) {
        if n == 1 {
            return Ok(if name.starts_with("bool") {
                Ty::Bool
            } else {
                Ty::F32
            });
        }
        return Ok(if name.starts_with("bool") {
            Ty::BVec(n)
        } else {
            Ty::Vec(n)
        });
    }
    if let Some(n) = parse::is_matrix_type(name) {
        return Ok(Ty::Mat(n));
    }
    Err(ShaderError::new("unsupported", format!("type `{name}`")))
}

/// The writable prologue locals — MilkDrop's own mutable shader vocabulary.
const WRITABLE: &[&str] = &["uv", "uv_orig", "rad", "ang", "ret", "hue_shader"];

/// The six `rot_*` families, in the uniform's row order.
const ROT_FAMILIES: &[&str] = &["s", "d", "f", "vf", "uf", "rand"];

pub fn emit(stage: Stage, unit: &Unit, tex_wrap: bool) -> Result<Translated, ShaderError> {
    let mut e = Emitter {
        stage,
        tex_wrap,
        scopes: Vec::new(),
        globals: HashMap::new(),
        user_fns: HashMap::new(),
        blur: 0,
        ops: 0,
        loops: 0,
        loop_depth: 0,
        tmp: 0,
        const_context: false,
        current_ret: Ty::Void,
        shadow_inits: Vec::new(),
    };

    // Sampler declarations: a builtin is declared already; anything else is a
    // disk texture, which is the deliberate exclusion this plan prices.
    for name in &unit.samplers {
        if e.sampler_pair(name).is_err() {
            return Err(disk_texture(name));
        }
    }

    let group = match stage {
        Stage::Warp => lmv_core::milk::shader::WARP_GROUP,
        Stage::Comp => lmv_core::milk::shader::COMP_GROUP,
    };
    let mut module = lmv_core::milk::shader::fragment_prelude(group);

    // Globals become module-scope `var<private>` — HLSL globals are mutable
    // statics. **Their initializers run at the top of `fs_main`**, not at
    // module scope: WGSL module initializers are compile-time, and the corpus
    // initializes globals from `time`, `rand_preset` and the q's constantly.
    // Deferring every initializer (not just the input-reading ones) keeps one
    // path, and for a per-fragment entry point the two are indistinguishable.
    for global in &unit.globals {
        let Stmt::Decl { ty, name, init } = global else {
            continue;
        };
        let ty = hlsl_ty(ty)?;
        module.push_str(&format!("var<private> m_{name}: {};\n", ty.wgsl()));
        e.globals.insert(name.clone(), ty);
        if let Some(expr) = init {
            let value = e.expr(expr)?;
            let text = e.convert(value, ty)?;
            e.shadow_inits.push((format!("m_{name}"), text));
        }
    }

    // fxc treats a shader input as an ordinary mutable global — presets assign
    // to `rand_preset`, `q` variables and the like, and shipped that way. WGSL
    // has no writable uniform, so each such input gets a `var<private>` shadow,
    // zero here and filled from the uniform at the top of `fs_main`; every
    // read and write then resolves to the shadow.
    {
        use std::collections::BTreeSet;
        let mut assigned = BTreeSet::new();
        let mut declared = BTreeSet::new();
        collect_names(&unit.body, &mut assigned, &mut declared);
        for func in &unit.funcs {
            collect_names(&func.body, &mut assigned, &mut declared);
            for (_, param) in &func.params {
                declared.insert(param.clone());
            }
        }
        for global in &unit.globals {
            if let Stmt::Decl { name, .. } = global {
                declared.insert(name.clone());
            }
        }
        for name in assigned {
            if declared.contains(&name) || WRITABLE.contains(&name.as_str()) {
                continue;
            }
            if let Some((expr, ty)) = e.builtin_ident(&name)
                && ty != Ty::Rot
            {
                module.push_str(&format!("var<private> m_{name}: {};\n", ty.wgsl()));
                e.globals.insert(name.clone(), ty);
                e.shadow_inits.push((format!("m_{name}"), expr));
            }
        }
    }

    // User helper functions, in declaration order (HLSL requires define-before-
    // use, so earlier ones are visible to later ones and to the body).
    for func in &unit.funcs {
        let text = e.function(func)?;
        module.push_str(&text);
    }

    module.push_str(&e.fs_main(&unit.body)?);

    if e.ops > MAX_OPS {
        return Err(ShaderError::new(
            "too-big",
            format!("{} static operations exceed the {MAX_OPS} cap", e.ops),
        ));
    }

    Ok(Translated {
        wgsl: module,
        blur_level: e.blur,
        ops: e.ops,
        loops: e.loops,
    })
}

fn disk_texture(name: &str) -> ShaderError {
    ShaderError::new(
        "disk-texture",
        format!("samples the user texture `{name}`, which is deliberately out of scope"),
    )
}

/// Every name the tree assigns to, and every name it declares — the difference
/// is the set of builtin inputs that need a mutable shadow.
fn collect_names(
    stmts: &[Stmt],
    assigned: &mut std::collections::BTreeSet<String>,
    declared: &mut std::collections::BTreeSet<String>,
) {
    fn base_ident(expr: &Expr) -> Option<&str> {
        match expr {
            Expr::Ident(name) => Some(name),
            Expr::Member(base, _) | Expr::Index(base, _) => base_ident(base),
            _ => None,
        }
    }
    for stmt in stmts {
        match stmt {
            Stmt::Decl { name, .. } => {
                declared.insert(name.clone());
            }
            Stmt::Assign { target, .. } => {
                if let Some(name) = base_ident(target) {
                    assigned.insert(name.to_string());
                }
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                collect_names(then, assigned, declared);
                collect_names(otherwise, assigned, declared);
            }
            Stmt::For {
                init, update, body, ..
            } => {
                collect_names(init, assigned, declared);
                collect_names(update, assigned, declared);
                collect_names(body, assigned, declared);
            }
            Stmt::While { body, .. } => collect_names(body, assigned, declared),
            _ => {}
        }
    }
}

struct Emitter {
    stage: Stage,
    tex_wrap: bool,
    scopes: Vec<HashMap<String, Ty>>,
    globals: HashMap<String, Ty>,
    user_fns: HashMap<String, (Ty, Vec<Ty>)>,
    blur: u8,
    ops: u32,
    loops: u32,
    loop_depth: u32,
    tmp: u32,
    /// Inside a global initializer, where a shader input cannot appear.
    const_context: bool,
    /// The return type of the function being emitted.
    current_ret: Ty,
    /// `(shadow, source)` pairs filled at the top of `fs_main` — the writable
    /// stand-ins for inputs the preset assigns to.
    shadow_inits: Vec<(String, String)>,
}

type Val = (String, Ty);

impl Emitter {
    fn lookup(&self, name: &str) -> Option<Ty> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(*ty);
            }
        }
        self.globals.get(name).copied()
    }

    fn declare(&mut self, name: &str, ty: Ty) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn next_tmp(&mut self) -> String {
        self.tmp += 1;
        format!("_lt{}", self.tmp)
    }

    // --- the input surface ---

    /// A read of a builtin name: the WGSL expression and its type.
    fn builtin_ident(&mut self, name: &str) -> Option<Val> {
        if self.const_context {
            return None;
        }
        let v = |s: &str, t: Ty| Some((s.to_string(), t));
        match name {
            "time" => v("U.clock.x", Ty::F32),
            "fps" => v("U.clock.y", Ty::F32),
            "frame" => v("U.clock.z", Ty::F32),
            "progress" => v("U.clock.w", Ty::F32),
            "bass" => v("U.bands.x", Ty::F32),
            "mid" => v("U.bands.y", Ty::F32),
            "treb" => v("U.bands.z", Ty::F32),
            "vol" => v("U.bands.w", Ty::F32),
            "bass_att" => v("U.bands_att.x", Ty::F32),
            "mid_att" => v("U.bands_att.y", Ty::F32),
            "treb_att" => v("U.bands_att.z", Ty::F32),
            "vol_att" => v("U.bands_att.w", Ty::F32),
            "texsize" => v("U.texsize", Ty::Vec(4)),
            "aspect" => v("U.aspect", Ty::Vec(4)),
            "rand_frame" => v("U.rand_frame", Ty::Vec(4)),
            "rand_preset" => v("U.rand_preset", Ty::Vec(4)),
            "decay" => v("U.misc.x", Ty::F32),
            "uv" => v("_lmv_uv", Ty::Vec(2)),
            "uv_orig" => v("_lmv_uv_orig", Ty::Vec(2)),
            "rad" => v("_lmv_rad", Ty::F32),
            "ang" => v("_lmv_ang", Ty::F32),
            "ret" => v("_lmv_ret", Ty::Vec(3)),
            "hue_shader" => v("_lmv_hue", Ty::Vec(3)),
            "slow_roam_cos" => v("U.roam[0]", Ty::Vec(4)),
            "roam_cos" => v("U.roam[1]", Ty::Vec(4)),
            "slow_roam_sin" => v("U.roam[2]", Ty::Vec(4)),
            "roam_sin" => v("U.roam[3]", Ty::Vec(4)),
            "texsize_noise_lq" | "texsize_noise_mq" | "texsize_noise_hq" => v(
                "vec4<f32>(256.0, 256.0, 0.00390625, 0.00390625)",
                Ty::Vec(4),
            ),
            "texsize_noise_lq_lite" | "texsize_noisevol_lq" | "texsize_noisevol_hq" => {
                v("vec4<f32>(32.0, 32.0, 0.03125, 0.03125)", Ty::Vec(4))
            }
            // MilkDrop's shader-header constants. `M_PI_2` is 2π in the
            // reference — not π/2 — and `M_INV_PI_2` its reciprocal.
            "M_PI" => v("3.14159265", Ty::F32),
            "M_PI_2" => v("6.28318531", Ty::F32),
            "M_INV_PI_2" => v("0.15915494", Ty::F32),
            _ => {
                // q1..q32 as scalars.
                if let Some(rest) = name.strip_prefix('q')
                    && let Ok(n) = rest.parse::<u32>()
                    && (1..=32).contains(&n)
                {
                    let i = (n - 1) / 4;
                    let c = ["x", "y", "z", "w"][((n - 1) % 4) as usize];
                    return Some((format!("U.q[{i}].{c}"), Ty::F32));
                }
                // _qa.._qh as vec4s.
                if let Some(rest) = name.strip_prefix("_q")
                    && rest.len() == 1
                    && let Some(c) = rest.chars().next()
                    && ('a'..='h').contains(&c)
                {
                    let i = c as u32 - 'a' as u32;
                    return Some((format!("U.q[{i}]"), Ty::Vec(4)));
                }
                // rot_s1..rot_rand4 — indexable rows.
                if let Some(rest) = name.strip_prefix("rot_") {
                    for (fi, family) in ROT_FAMILIES.iter().enumerate() {
                        if let Some(num) = rest.strip_prefix(family)
                            && let Ok(k) = num.parse::<usize>()
                            && (1..=4).contains(&k)
                        {
                            let base = (fi * 4 + (k - 1)) * 4;
                            return Some((base.to_string(), Ty::Rot));
                        }
                    }
                }
                None
            }
        }
    }

    /// The `(texture, sampler, dimensions)` a sampler name binds.
    ///
    /// MilkDrop composes sampler names: an optional filter/address prefix
    /// (`fw`/`fc`/`pw`/`pc`) before **any** built-in texture — `sampler_main`,
    /// `sampler_pw_noise_lq`, `sampler_fc_blur1` are all one grammar. A base
    /// that is not a built-in is a disk texture, the deliberate exclusion.
    fn sampler_pair(
        &mut self,
        name: &str,
    ) -> Result<(&'static str, &'static str, u8), ShaderError> {
        let Some(rest) = name.strip_prefix("sampler_") else {
            return Err(ShaderError::new(
                "unknown-name",
                format!("unknown sampler `{name}`"),
            ));
        };
        let (chosen, base) = match rest.split_once('_') {
            Some(("fw", base)) => (Some("s_fw"), base),
            Some(("fc", base)) => (Some("s_fc"), base),
            Some(("pw", base)) => (Some("s_pw"), base),
            Some(("pc", base)) => (Some("s_pc"), base),
            _ => (None, rest),
        };
        // A bare prefix (`sampler_fw`) means the main texture.
        let base = match base {
            "" | "fw" | "fc" | "pw" | "pc" if chosen.is_none() => {
                return self.sampler_pair(&format!("sampler_{base}_main"));
            }
            other => other,
        };
        let main_samp = if self.tex_wrap { "s_fw" } else { "s_fc" };
        let (tex, default_samp, dims): (&'static str, &'static str, u8) = match base {
            "main" => ("t_main", main_samp, 2),
            "blur1" => {
                self.blur = self.blur.max(1);
                ("t_blur1", "s_fc", 2)
            }
            "blur2" => {
                self.blur = self.blur.max(2);
                ("t_blur2", "s_fc", 2)
            }
            "blur3" => {
                self.blur = self.blur.max(3);
                ("t_blur3", "s_fc", 2)
            }
            "noise_lq" => ("t_noise_lq", "s_fw", 2),
            "noise_lq_lite" => ("t_noise_lq_lite", "s_fw", 2),
            "noise_mq" => ("t_noise_mq", "s_fw", 2),
            "noise_hq" => ("t_noise_hq", "s_fw", 2),
            "noisevol_lq" => ("t_noisevol_lq", "s_fw", 3),
            "noisevol_hq" => ("t_noisevol_hq", "s_fw", 3),
            other => return Err(disk_texture(other)),
        };
        Ok((tex, chosen.unwrap_or(default_samp), dims))
    }

    // --- conversions ---

    /// Coerce `value` to `want`, the way legacy fxc did: splat a scalar,
    /// truncate a longer vector, promote a bool.
    fn convert(&mut self, value: Val, want: Ty) -> Result<String, ShaderError> {
        let (text, have) = value;
        if have == want {
            return Ok(text);
        }
        match (have, want) {
            (Ty::Bool, Ty::F32) => Ok(format!("f32({text})")),
            (Ty::BVec(n), Ty::Vec(m)) if n == m => Ok(format!("vec{n}<f32>({text})")),
            (Ty::F32, Ty::Vec(n)) => Ok(format!("vec{n}<f32>({text})")),
            (Ty::Bool, Ty::Vec(n)) => Ok(format!("vec{n}<f32>(f32({text}))")),
            (Ty::Vec(m), Ty::Vec(n)) if m > n => Ok(format!("({text}).{}", &"xyzw"[..n as usize])),
            // Widening, zero-padded. Legacy fxc accepted a narrower vector
            // where a wider one was declared (140 corpus files lean on it, most
            // via `lum(float2)`), and padding with zero is the reading that
            // contributes nothing where nothing was given.
            (Ty::Vec(m), Ty::Vec(n)) if m < n => {
                let pad = ", 0.0".repeat((n - m) as usize);
                Ok(format!("vec{n}<f32>({text}{pad})"))
            }
            (Ty::Vec(_), Ty::F32) => Ok(format!("({text}).x")),
            (Ty::F32, Ty::Bool) => Ok(format!("(({text}) != 0.0)")),
            (Ty::Vec(_), Ty::Bool) => Ok(format!("(({text}).x != 0.0)")),
            (Ty::Bool, Ty::BVec(n)) => Ok(format!("vec{n}<bool>({text})")),
            // A bool vector truncates like a float one: through floats, then
            // the lane subset — fxc's own reading of `float k = (a == b);`.
            (Ty::BVec(_), Ty::F32) => Ok(format!("f32(({text}).x)")),
            (Ty::BVec(m), Ty::Vec(n)) if m > n => {
                Ok(format!("(vec{m}<f32>({text})).{}", &"xyzw"[..n as usize]))
            }
            _ => Err(ShaderError::new(
                "unsupported",
                format!("no conversion from {} to {}", have.wgsl(), want.wgsl()),
            )),
        }
    }

    fn coerce_bool(&mut self, value: Val) -> Result<String, ShaderError> {
        let (text, ty) = value;
        match ty {
            Ty::Bool => Ok(text),
            Ty::F32 => Ok(format!("(({text}) != 0.0)")),
            Ty::BVec(_) => Ok(format!("all({text})")),
            Ty::Vec(n) => Ok(format!("all(({text}) != vec{n}<f32>(0.0))")),
            other => Err(ShaderError::new(
                "unsupported",
                format!("a {} as a condition", other.wgsl()),
            )),
        }
    }

    /// Numeric-ify a value for arithmetic: bools become floats.
    fn arith(&mut self, value: Val) -> Result<Val, ShaderError> {
        match value.1 {
            Ty::Bool => Ok((format!("f32({})", value.0), Ty::F32)),
            Ty::BVec(n) => Ok((format!("vec{n}<f32>({})", value.0), Ty::Vec(n))),
            Ty::Void | Ty::Rot => Err(ShaderError::new(
                "unsupported",
                "a rot matrix or void value in arithmetic",
            )),
            _ => Ok(value),
        }
    }

    /// Promote a set of values to one common componentwise type — the smaller
    /// vector size wins (fxc's truncation), a lone scalar splats.
    fn promote_common(&mut self, values: Vec<Val>) -> Result<(Vec<String>, Ty), ShaderError> {
        let values: Vec<Val> = values
            .into_iter()
            .map(|v| self.arith(v))
            .collect::<Result<_, _>>()?;
        let mut size: Option<u8> = None;
        for (_, ty) in &values {
            match ty {
                Ty::Vec(n) => size = Some(size.map_or(*n, |s| s.min(*n))),
                Ty::F32 => {}
                other => {
                    return Err(ShaderError::new(
                        "unsupported",
                        format!("a {} argument to a componentwise function", other.wgsl()),
                    ));
                }
            }
        }
        let want = size.map_or(Ty::F32, Ty::Vec);
        let out = values
            .into_iter()
            .map(|v| self.convert(v, want))
            .collect::<Result<_, _>>()?;
        Ok((out, want))
    }

    // --- expressions ---

    fn expr(&mut self, expr: &Expr) -> Result<Val, ShaderError> {
        self.ops += 1;
        match expr {
            Expr::Num(text) => Ok((number(text)?, Ty::F32)),
            Expr::Ident(name) => {
                if let Some(ty) = self.lookup(name) {
                    return Ok((format!("m_{name}"), ty));
                }
                if self.const_context {
                    // Inside a global initializer no shader input exists yet —
                    // WGSL module-scope initializers are compile-time.
                    return Err(ShaderError::new(
                        "unsupported",
                        format!("a global initializer reads `{name}`"),
                    ));
                }
                if let Some(v) = self.builtin_ident(name) {
                    return Ok(v);
                }
                Err(unknown(name))
            }
            Expr::Unary(op, inner) => {
                let value = self.expr(inner)?;
                match *op {
                    "!" => {
                        let b = self.coerce_bool(value)?;
                        Ok((format!("(!{b})"), Ty::Bool))
                    }
                    _ => {
                        let (text, ty) = self.arith(value)?;
                        match ty {
                            Ty::Mat(_) => Ok((format!("(({text}) * -1.0)"), ty)),
                            _ => Ok((format!("(-({text}))"), ty)),
                        }
                    }
                }
            }
            Expr::Binary(op, left, right) => {
                let l = self.expr(left)?;
                let r = self.expr(right)?;
                self.binary(op, l, r)
            }
            Expr::Ternary(cond, then, otherwise) => {
                let cond = self.expr(cond)?;
                let t = self.expr(then)?;
                let f = self.expr(otherwise)?;
                // `select` picks componentwise under a vector condition, which
                // is HLSL's own vector `?:`.
                let (cond_text, want) = match cond.1 {
                    Ty::BVec(n) => (cond.0.clone(), Some(n)),
                    _ => (self.coerce_bool(cond)?, None),
                };
                let (mut pair, mut ty) = self.promote_common(vec![t, f])?;
                if let Some(n) = want
                    && ty != Ty::Vec(n)
                {
                    pair = pair
                        .into_iter()
                        .map(|p| self.convert((p, ty), Ty::Vec(n)))
                        .collect::<Result<_, _>>()?;
                    ty = Ty::Vec(n);
                }
                let f_text = pair.pop().unwrap_or_default();
                let t_text = pair.pop().unwrap_or_default();
                Ok((format!("select({f_text}, {t_text}, {cond_text})"), ty))
            }
            Expr::Call(name, args) => self.call(name, args),
            Expr::Cast(ty_name, inner) => {
                let want = hlsl_ty(ty_name)?;
                let value = self.expr(inner)?;
                let text = self.convert(value, want)?;
                Ok((text, want))
            }
            Expr::Member(base, member) => {
                let (text, ty) = self.expr(base)?;
                swizzle((text, ty), member)
            }
            Expr::Index(base, index) => {
                let (text, ty) = self.expr(base)?;
                let idx = self.expr(index)?;
                let idx_text = index_text(&idx.0);
                match ty {
                    Ty::Rot => Ok((format!("U.rot[{text} + {idx_text}].xyz"), Ty::Vec(3))),
                    Ty::Vec(_) => Ok((format!("({text})[{idx_text}]"), Ty::F32)),
                    other => Err(ShaderError::new(
                        "unsupported",
                        format!("indexing a {}", other.wgsl()),
                    )),
                }
            }
        }
    }

    fn binary(&mut self, op: &str, l: Val, r: Val) -> Result<Val, ShaderError> {
        match op {
            "&&" | "||" => {
                let lb = self.coerce_bool(l)?;
                let rb = self.coerce_bool(r)?;
                Ok((format!("(({lb}) {op} ({rb}))"), Ty::Bool))
            }
            "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                let l = self.arith(l)?;
                let r = self.arith(r)?;
                match (l.1, r.1) {
                    (Ty::F32, Ty::F32) => Ok((format!("(({}) {op} ({}))", l.0, r.0), Ty::Bool)),
                    _ => {
                        let (pair, ty) = self.promote_common(vec![l, r])?;
                        let n = match ty {
                            Ty::Vec(n) => n,
                            _ => {
                                let a = pair.first().cloned().unwrap_or_default();
                                let b = pair.get(1).cloned().unwrap_or_default();
                                return Ok((format!("(({a}) {op} ({b}))"), Ty::Bool));
                            }
                        };
                        let a = pair.first().cloned().unwrap_or_default();
                        let b = pair.get(1).cloned().unwrap_or_default();
                        Ok((format!("(({a}) {op} ({b}))"), Ty::BVec(n)))
                    }
                }
            }
            "+" | "-" | "*" | "/" | "%" => {
                let l = self.arith(l)?;
                let r = self.arith(r)?;
                match (l.1, r.1) {
                    // Matrices: `+`/`-` are componentwise in both languages;
                    // scalar `*` scales; everything else is a named rejection —
                    // HLSL's `m1 * m2` is componentwise where WGSL's is the
                    // product, and silently changing the math is the one error
                    // this module must not make.
                    (Ty::Mat(a), Ty::Mat(b)) if a == b && (op == "+" || op == "-") => {
                        Ok((format!("(({}) {op} ({}))", l.0, r.0), Ty::Mat(a)))
                    }
                    (Ty::Mat(a), Ty::F32) if op == "*" => {
                        Ok((format!("(({}) * ({}))", l.0, r.0), Ty::Mat(a)))
                    }
                    (Ty::Mat(a), Ty::F32) if op == "/" => {
                        Ok((format!("(({}) * (1.0 / ({})))", l.0, r.0), Ty::Mat(a)))
                    }
                    (Ty::F32, Ty::Mat(a)) if op == "*" => {
                        Ok((format!("(({}) * ({}))", l.0, r.0), Ty::Mat(a)))
                    }
                    (Ty::Mat(_), _) | (_, Ty::Mat(_)) => Err(ShaderError::new(
                        "unsupported",
                        format!("`{op}` between {} and {}", l.1.wgsl(), r.1.wgsl()),
                    )),
                    (Ty::F32, Ty::F32) => Ok((format!("(({}) {op} ({}))", l.0, r.0), Ty::F32)),
                    (Ty::Vec(n), Ty::F32) | (Ty::F32, Ty::Vec(n)) => {
                        Ok((format!("(({}) {op} ({}))", l.0, r.0), Ty::Vec(n)))
                    }
                    (Ty::Vec(a), Ty::Vec(b)) => {
                        if a == b {
                            Ok((format!("(({}) {op} ({}))", l.0, r.0), Ty::Vec(a)))
                        } else {
                            let (pair, ty) = self.promote_common(vec![l, r])?;
                            let x = pair.first().cloned().unwrap_or_default();
                            let y = pair.get(1).cloned().unwrap_or_default();
                            Ok((format!("(({x}) {op} ({y}))"), ty))
                        }
                    }
                    _ => Err(ShaderError::new(
                        "unsupported",
                        format!("`{op}` between {} and {}", l.1.wgsl(), r.1.wgsl()),
                    )),
                }
            }
            other => Err(ShaderError::new(
                "unsupported",
                format!("operator `{other}`"),
            )),
        }
    }

    fn call(&mut self, name: &str, args: &[Expr]) -> Result<Val, ShaderError> {
        // Type constructors first: `float3(...)` is a call whose name is a type.
        if parse::is_type_name(name) {
            return self.constructor(name, args);
        }
        // User functions shadow intrinsics — a preset defining its own `lum`
        // means its own.
        if let Some((ret, params)) = self.user_fns.get(name).cloned() {
            if params.len() != args.len() {
                return Err(ShaderError::new(
                    "unsupported",
                    format!(
                        "`{name}` called with {} arguments, takes {}",
                        args.len(),
                        params.len()
                    ),
                ));
            }
            let mut parts = Vec::new();
            for (arg, want) in args.iter().zip(params) {
                let value = self.expr(arg)?;
                parts.push(self.convert(value, want)?);
            }
            return Ok((format!("m_{name}({})", parts.join(", ")), ret));
        }
        match name {
            // Case-insensitively: the reference's compiler accepted `tex2d`,
            // and 17 files in the original pack alone spell it that way.
            "tex2D" | "tex3D" | "tex2Dlod" | "tex2Dbias" | "tex2d" | "tex3d" | "tex2dlod"
            | "tex2dbias" => self.tex(name, args),
            "GetPixel" | "GetMain" => {
                let uv = self.arg(args, 0, name)?;
                let uv = self.convert(uv, Ty::Vec(2))?;
                Ok((format!("lmv_GetPixel({uv})"), Ty::Vec(3)))
            }
            "GetBlur1" | "GetBlur2" | "GetBlur3" => {
                let level = name.as_bytes().last().map_or(1, |b| b - b'0');
                self.blur = self.blur.max(level);
                let uv = self.arg(args, 0, name)?;
                let uv = self.convert(uv, Ty::Vec(2))?;
                Ok((format!("lmv_{name}({uv})"), Ty::Vec(3)))
            }
            "lum" => {
                let c = self.arg(args, 0, name)?;
                let c = self.convert(c, Ty::Vec(3))?;
                Ok((format!("lmv_lum({c})"), Ty::F32))
            }
            "mul" => {
                let l = self.arg(args, 0, name)?;
                let r = self.arg(args, 1, name)?;
                match (l.1, r.1) {
                    (Ty::Mat(n), Ty::Vec(m)) | (Ty::Vec(m), Ty::Mat(n)) if n == m => {
                        Ok((format!("(({}) * ({}))", l.0, r.0), Ty::Vec(n)))
                    }
                    (Ty::Mat(a), Ty::Mat(b)) if a == b => {
                        Ok((format!("(({}) * ({}))", l.0, r.0), Ty::Mat(a)))
                    }
                    (Ty::F32, _) | (_, Ty::F32) => self.binary("*", l, r),
                    (Ty::Vec(a), Ty::Vec(b)) if a == b => {
                        Ok((format!("dot({}, {})", l.0, r.0), Ty::F32))
                    }
                    _ => Err(ShaderError::new(
                        "unsupported",
                        format!("mul({}, {})", l.1.wgsl(), r.1.wgsl()),
                    )),
                }
            }
            "transpose" => {
                let m = self.arg(args, 0, name)?;
                match m.1 {
                    Ty::Mat(_) => Ok((format!("transpose({})", m.0), m.1)),
                    other => Err(ShaderError::new(
                        "unsupported",
                        format!("transpose of {}", other.wgsl()),
                    )),
                }
            }
            "dot" => {
                let (pair, ty) = self.promoted_pair(args, name)?;
                match ty {
                    Ty::Vec(_) => Ok((format!("dot({}, {})", pair.0, pair.1), Ty::F32)),
                    Ty::F32 => Ok((format!("(({}) * ({}))", pair.0, pair.1), Ty::F32)),
                    _ => Err(ShaderError::new("unsupported", "dot of non-vectors")),
                }
            }
            "cross" => {
                let l = self.arg(args, 0, name)?;
                let r = self.arg(args, 1, name)?;
                let l = self.convert(l, Ty::Vec(3))?;
                let r = self.convert(r, Ty::Vec(3))?;
                Ok((format!("cross({l}, {r})"), Ty::Vec(3)))
            }
            "length" => {
                let v = self.arg(args, 0, name)?;
                match v.1 {
                    Ty::Vec(_) => Ok((format!("length({})", v.0), Ty::F32)),
                    _ => {
                        let s = self.convert(v, Ty::F32)?;
                        Ok((format!("abs({s})"), Ty::F32))
                    }
                }
            }
            "distance" => {
                let (pair, ty) = self.promoted_pair(args, name)?;
                match ty {
                    Ty::Vec(_) => Ok((format!("distance({}, {})", pair.0, pair.1), Ty::F32)),
                    _ => Ok((format!("abs(({}) - ({}))", pair.0, pair.1), Ty::F32)),
                }
            }
            "normalize" | "reflect" | "refract" => {
                // Vector-space intrinsics: same names, same shapes — except
                // that fxc accepts a scalar `normalize(s)`, which is `s/|s|`,
                // i.e. sign.
                let first = self.arg(args, 0, name)?;
                if name == "normalize" && matches!(first.1, Ty::F32 | Ty::Bool) {
                    let s = self.convert(first, Ty::F32)?;
                    return Ok((format!("sign({s})"), Ty::F32));
                }
                let ty = first.1;
                let mut parts = vec![self.convert(first, ty)?];
                for (i, arg) in args.iter().enumerate().skip(1) {
                    let value = self.expr(arg)?;
                    let want = if name == "refract" && i == 2 {
                        Ty::F32
                    } else {
                        ty
                    };
                    parts.push(self.convert(value, want)?);
                }
                Ok((format!("{name}({})", parts.join(", ")), ty))
            }
            "any" | "all" => {
                let v = self.expr(self.arg_expr(args, 0, name)?)?;
                let inner = match v.1 {
                    Ty::BVec(_) => v.0,
                    Ty::Vec(n) => format!("(({}) != vec{n}<f32>(0.0))", v.0),
                    Ty::Bool => return Ok((v.0, Ty::Bool)),
                    Ty::F32 => return Ok((format!("(({}) != 0.0)", v.0), Ty::Bool)),
                    other => {
                        return Err(ShaderError::new(
                            "unsupported",
                            format!("{name} of {}", other.wgsl()),
                        ));
                    }
                };
                Ok((format!("{name}({inner})"), Ty::Bool))
            }
            // HLSL's `atan(y, x)` two-argument overload is `atan2`.
            "atan2" | "atan" if args.len() == 2 => {
                let (pair, ty) = self.promoted_pair(args, name)?;
                Ok((format!("atan2({}, {})", pair.0, pair.1), ty))
            }
            "fmod" => {
                let (pair, ty) = self.promoted_pair(args, name)?;
                Ok((format!("(({}) % ({}))", pair.0, pair.1), ty))
            }
            "rcp" => {
                let v = self.arg(args, 0, name)?;
                let (text, ty) = self.arith(v)?;
                Ok((format!("(1.0 / ({text}))"), ty))
            }
            "log10" => {
                let v = self.arg(args, 0, name)?;
                let (text, ty) = self.arith(v)?;
                Ok((format!("(log({text}) * 0.4342944819)"), ty))
            }
            "mad" => {
                let (parts, ty) = self.promoted_all(args, name)?;
                Ok((format!("fma({})", parts.join(", ")), ty))
            }
            "lerp" => {
                let (parts, ty) = self.promoted_all(args, name)?;
                Ok((format!("mix({})", parts.join(", ")), ty))
            }
            "frac" => self.componentwise("fract", args, name),
            "rsqrt" => self.componentwise("inverseSqrt", args, name),
            "saturate" | "sqrt" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "exp"
            | "log" | "exp2" | "log2" | "floor" | "ceil" | "abs" | "sign" | "trunc" | "round"
            | "degrees" | "radians" | "sinh" | "cosh" | "tanh" => {
                self.componentwise(name, args, name)
            }
            "pow" | "min" | "max" | "step" | "clamp" | "smoothstep" => {
                let (parts, ty) = self.promoted_all(args, name)?;
                Ok((format!("{name}({})", parts.join(", ")), ty))
            }
            other => Err(unknown(other)),
        }
    }

    fn arg_expr<'e>(
        &self,
        args: &'e [Expr],
        index: usize,
        name: &str,
    ) -> Result<&'e Expr, ShaderError> {
        args.get(index).ok_or_else(|| {
            ShaderError::new(
                "unsupported",
                format!("`{name}` is missing argument {}", index + 1),
            )
        })
    }

    fn arg(&mut self, args: &[Expr], index: usize, name: &str) -> Result<Val, ShaderError> {
        let expr = self.arg_expr(args, index, name)?.clone();
        self.expr(&expr)
    }

    fn promoted_pair(
        &mut self,
        args: &[Expr],
        name: &str,
    ) -> Result<((String, String), Ty), ShaderError> {
        let l = self.arg(args, 0, name)?;
        let r = self.arg(args, 1, name)?;
        let (mut pair, ty) = self.promote_common(vec![l, r])?;
        let b = pair.pop().unwrap_or_default();
        let a = pair.pop().unwrap_or_default();
        Ok(((a, b), ty))
    }

    fn promoted_all(
        &mut self,
        args: &[Expr],
        name: &str,
    ) -> Result<(Vec<String>, Ty), ShaderError> {
        let values = args
            .iter()
            .map(|a| self.expr(a))
            .collect::<Result<Vec<_>, _>>()?;
        if values.is_empty() {
            return Err(ShaderError::new(
                "unsupported",
                format!("`{name}` with no arguments"),
            ));
        }
        self.promote_common(values)
    }

    fn componentwise(&mut self, wgsl: &str, args: &[Expr], name: &str) -> Result<Val, ShaderError> {
        let v = self.arg(args, 0, name)?;
        let (text, ty) = self.arith(v)?;
        match ty {
            Ty::F32 | Ty::Vec(_) => Ok((format!("{wgsl}({text})"), ty)),
            other => Err(ShaderError::new(
                "unsupported",
                format!("{name} of {}", other.wgsl()),
            )),
        }
    }

    fn tex(&mut self, name: &str, args: &[Expr]) -> Result<Val, ShaderError> {
        let Some(Expr::Ident(sampler)) = args.first() else {
            return Err(ShaderError::new(
                "unsupported",
                format!("`{name}` needs a literal sampler name as its first argument"),
            ));
        };
        let (tex, samp, dims) = self.sampler_pair(sampler)?;
        let coords = self.arg(args, 1, name)?;
        match name.to_ascii_lowercase().as_str() {
            // `tex2Dlod` packs the level into `.w` of a float4.
            "tex2dlod" => {
                let c = self.convert(coords, Ty::Vec(4))?;
                Ok((
                    format!("textureSampleLevel({tex}, {samp}, ({c}).xy, ({c}).w)"),
                    Ty::Vec(4),
                ))
            }
            _ => {
                let want = if dims == 3 { Ty::Vec(3) } else { Ty::Vec(2) };
                let c = self.convert(coords, want)?;
                // Level 0 everywhere: no MilkDrop texture has mips, and implicit
                // derivatives would trip naga's uniformity analysis inside the
                // conditionals presets sample from (module docs).
                Ok((
                    format!("textureSampleLevel({tex}, {samp}, {c}, 0.0)"),
                    Ty::Vec(4),
                ))
            }
        }
    }

    fn constructor(&mut self, name: &str, args: &[Expr]) -> Result<Val, ShaderError> {
        if let Some(n) = parse::is_matrix_type(name) {
            // Arguments flatten into row-major components — scalars, vectors
            // and any mix, which is fxc's own rule — then the rows go through
            // `transpose` so the mathematical matrix is HLSL's (module docs,
            // choice 2). A vector argument's components are extracted by
            // swizzle; the operands are pure, so re-evaluation is only cost.
            let values = args
                .iter()
                .map(|a| self.expr(a))
                .collect::<Result<Vec<_>, _>>()?;
            let mut scalars = Vec::new();
            for v in values {
                let v = self.arith(v)?;
                match v.1 {
                    Ty::F32 => scalars.push(v.0),
                    Ty::Vec(k) => {
                        for lane in 0..k {
                            scalars.push(format!("({}).{}", v.0, lane_name(lane)));
                        }
                    }
                    other => {
                        return Err(ShaderError::new(
                            "unsupported",
                            format!("a {} inside `{name}(...)`", other.wgsl()),
                        ));
                    }
                }
            }
            let n_us = n as usize;
            if scalars.len() != n_us * n_us {
                return Err(ShaderError::new(
                    "unsupported",
                    format!("`{name}` constructed from {} components", scalars.len()),
                ));
            }
            let rows: Vec<String> = scalars
                .chunks(n_us)
                .map(|row| format!("vec{n}<f32>({})", row.join(", ")))
                .collect();
            return Ok((
                format!("transpose(mat{n}x{n}<f32>({}))", rows.join(", ")),
                Ty::Mat(n),
            ));
        }
        let ty = hlsl_ty(name)?;
        let want_n = match ty {
            Ty::F32 | Ty::Bool => {
                let v = self.arg(args, 0, name)?;
                let text = self.convert(v, ty)?;
                return Ok((text, ty));
            }
            Ty::Vec(n) | Ty::BVec(n) => n,
            _ => {
                return Err(ShaderError::new(
                    "unsupported",
                    format!("constructor `{name}`"),
                ));
            }
        };
        let values = args
            .iter()
            .map(|a| self.expr(a))
            .collect::<Result<Vec<_>, _>>()?;
        // A single argument splats or truncates.
        if let [one] = values.as_slice() {
            let text = self.convert(one.clone(), Ty::Vec(want_n))?;
            return Ok((text, Ty::Vec(want_n)));
        }
        // Otherwise components flatten in order, exactly as both languages do.
        let mut total = 0u8;
        let mut parts = Vec::new();
        for v in values {
            let v = self.arith(v)?;
            total += match v.1 {
                Ty::F32 => 1,
                Ty::Vec(n) => n,
                other => {
                    return Err(ShaderError::new(
                        "unsupported",
                        format!("a {} inside `{name}(...)`", other.wgsl()),
                    ));
                }
            };
            parts.push(v.0);
        }
        if total != want_n {
            return Err(ShaderError::new(
                "unsupported",
                format!("`{name}` constructed from {total} components"),
            ));
        }
        Ok((
            format!("vec{want_n}<f32>({})", parts.join(", ")),
            Ty::Vec(want_n),
        ))
    }

    // --- statements ---

    fn stmts(&mut self, list: &[Stmt], out: &mut String, depth: usize) -> Result<(), ShaderError> {
        for stmt in list {
            self.stmt(stmt, out, depth)?;
        }
        Ok(())
    }

    fn stmt(&mut self, stmt: &Stmt, out: &mut String, depth: usize) -> Result<(), ShaderError> {
        let pad = "    ".repeat(depth);
        match stmt {
            Stmt::Decl { ty, name, init } => {
                let ty = hlsl_ty(ty)?;
                if ty == Ty::Void {
                    return Err(ShaderError::new("unsupported", "a void variable"));
                }
                let text = match init {
                    Some(expr) => {
                        let v = self.expr(expr)?;
                        self.convert(v, ty)?
                    }
                    None => ty.zero(),
                };
                out.push_str(&format!("{pad}var m_{name}: {} = {text};\n", ty.wgsl()));
                self.declare(name, ty);
            }
            Stmt::Assign { target, op, value } => {
                self.assign(target, *op, value, out, depth)?;
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                let c = self.expr(cond)?;
                let c = self.coerce_bool(c)?;
                out.push_str(&format!("{pad}if {c} {{\n"));
                self.scoped(then, out, depth + 1)?;
                if otherwise.is_empty() {
                    out.push_str(&format!("{pad}}}\n"));
                } else {
                    out.push_str(&format!("{pad}}} else {{\n"));
                    self.scoped(otherwise, out, depth + 1)?;
                    out.push_str(&format!("{pad}}}\n"));
                }
            }
            Stmt::For {
                init,
                cond,
                update,
                body,
            } => {
                self.enter_loop()?;
                self.scopes.push(HashMap::new());
                out.push_str(&format!("{pad}{{\n"));
                let inner = "    ".repeat(depth + 1);
                self.stmts(init, out, depth + 1)?;
                let guard = self.next_tmp();
                self.loops += 1;
                out.push_str(&format!("{inner}var {guard}: i32 = 0;\n"));
                let cond_text = match cond {
                    Some(c) => {
                        let v = self.expr(c)?;
                        self.coerce_bool(v)?
                    }
                    None => "true".into(),
                };
                out.push_str(&format!("{inner}loop {{\n"));
                let body_pad = "    ".repeat(depth + 2);
                out.push_str(&format!(
                    "{body_pad}if !(({cond_text}) && ({guard} < {LOOP_CAP})) {{ break; }}\n\
                     {body_pad}{guard} = {guard} + 1;\n"
                ));
                self.scoped(body, out, depth + 2)?;
                if update.is_empty() {
                    out.push_str(&format!("{body_pad}continuing {{ }}\n"));
                } else {
                    out.push_str(&format!("{body_pad}continuing {{\n"));
                    self.stmts(update, out, depth + 3)?;
                    out.push_str(&format!("{body_pad}}}\n"));
                }
                out.push_str(&format!("{inner}}}\n{pad}}}\n"));
                self.scopes.pop();
                self.loop_depth -= 1;
            }
            Stmt::While { cond, body } => {
                self.enter_loop()?;
                self.scopes.push(HashMap::new());
                out.push_str(&format!("{pad}{{\n"));
                let inner = "    ".repeat(depth + 1);
                let guard = self.next_tmp();
                self.loops += 1;
                out.push_str(&format!("{inner}var {guard}: i32 = 0;\n"));
                let c = self.expr(cond)?;
                let cond_text = self.coerce_bool(c)?;
                out.push_str(&format!("{inner}loop {{\n"));
                let body_pad = "    ".repeat(depth + 2);
                out.push_str(&format!(
                    "{body_pad}if !(({cond_text}) && ({guard} < {LOOP_CAP})) {{ break; }}\n\
                     {body_pad}{guard} = {guard} + 1;\n"
                ));
                self.scoped(body, out, depth + 2)?;
                out.push_str(&format!("{inner}}}\n{pad}}}\n"));
                self.scopes.pop();
                self.loop_depth -= 1;
            }
            Stmt::Expr(expr) => {
                let (text, ty) = self.expr(expr)?;
                if ty == Ty::Void {
                    out.push_str(&format!("{pad}{text};\n"));
                } else {
                    out.push_str(&format!("{pad}_ = {text};\n"));
                }
            }
            Stmt::Return(value) => {
                if self.current_ret == Ty::Void && value.is_none() {
                    out.push_str(&format!("{pad}return;\n"));
                } else {
                    let want = self.current_ret;
                    if want == Ty::Void {
                        return Err(ShaderError::new(
                            "unsupported",
                            "`return` inside shader_body",
                        ));
                    }
                    let Some(value) = value else {
                        return Err(ShaderError::new(
                            "unsupported",
                            "`return;` in a value-returning function",
                        ));
                    };
                    let v = self.expr(value)?;
                    let text = self.convert(v, want)?;
                    out.push_str(&format!("{pad}return {text};\n"));
                }
            }
            Stmt::Break => out.push_str(&format!("{pad}break;\n")),
            Stmt::Continue => out.push_str(&format!("{pad}continue;\n")),
        }
        Ok(())
    }

    fn enter_loop(&mut self) -> Result<(), ShaderError> {
        self.loop_depth += 1;
        if self.loop_depth > MAX_LOOP_DEPTH {
            return Err(ShaderError::new(
                "too-big",
                format!("loops nested deeper than {MAX_LOOP_DEPTH}"),
            ));
        }
        Ok(())
    }

    fn scoped(&mut self, body: &[Stmt], out: &mut String, depth: usize) -> Result<(), ShaderError> {
        self.scopes.push(HashMap::new());
        let result = self.stmts(body, out, depth);
        self.scopes.pop();
        result
    }

    /// An assignment, including HLSL's write-masked swizzle forms WGSL has no
    /// single statement for.
    fn assign(
        &mut self,
        target: &Expr,
        op: Option<&'static str>,
        value: &Expr,
        out: &mut String,
        depth: usize,
    ) -> Result<(), ShaderError> {
        let pad = "    ".repeat(depth);
        match target {
            Expr::Ident(name) => {
                let (text, ty) = self.lvalue_ident(name)?;
                let rhs = self.rhs((text.clone(), ty), op, value)?;
                let rhs = self.convert(rhs, ty)?;
                out.push_str(&format!("{pad}{text} = {rhs};\n"));
                Ok(())
            }
            Expr::Member(base, member) => {
                let Expr::Ident(base_name) = base.as_ref() else {
                    return Err(ShaderError::new(
                        "unsupported",
                        "assignment through a nested member",
                    ));
                };
                let (base_text, base_ty) = self.lvalue_ident(base_name)?;
                let Ty::Vec(n) = base_ty else {
                    return Err(ShaderError::new(
                        "unsupported",
                        format!("member assignment on a {}", base_ty.wgsl()),
                    ));
                };
                let lanes = swizzle_lanes(member, n)?;
                let current = swizzle((base_text.clone(), base_ty), member)?;
                let want = current.1;
                let rhs = self.rhs(current, op, value)?;
                let rhs = self.convert(rhs, want)?;
                if let [lane] = lanes.as_slice() {
                    out.push_str(&format!("{pad}{base_text}.{} = {rhs};\n", lane_name(*lane)));
                    return Ok(());
                }
                // Identity swizzle over the whole vector is a plain assignment.
                let identity = lanes.len() == n as usize
                    && lanes.iter().enumerate().all(|(i, l)| *l as usize == i);
                if identity {
                    out.push_str(&format!("{pad}{base_text} = {rhs};\n"));
                    return Ok(());
                }
                // A write mask: stage the value once, then one lane at a time.
                let tmp = self.next_tmp();
                out.push_str(&format!("{pad}{{ let {tmp} = {rhs};\n"));
                for (i, lane) in lanes.iter().enumerate() {
                    let src = lane_name(i as u8);
                    out.push_str(&format!(
                        "{pad}  {base_text}.{} = {tmp}.{src};\n",
                        lane_name(*lane)
                    ));
                }
                out.push_str(&format!("{pad}}}\n"));
                Ok(())
            }
            _ => Err(ShaderError::new(
                "unsupported",
                "assignment to something that is not a variable or swizzle",
            )),
        }
    }

    fn rhs(
        &mut self,
        current: Val,
        op: Option<&'static str>,
        value: &Expr,
    ) -> Result<Val, ShaderError> {
        let v = self.expr(value)?;
        match op {
            None => Ok(v),
            Some(op) => self.binary(op, current, v),
        }
    }

    /// Resolve an assignment target name: a declared local/global, or one of
    /// the writable prologue locals. Writing any other input is refused with the
    /// name — MilkDrop's own compiler refuses it too.
    fn lvalue_ident(&mut self, name: &str) -> Result<(String, Ty), ShaderError> {
        if let Some(ty) = self.lookup(name) {
            return Ok((format!("m_{name}"), ty));
        }
        if WRITABLE.contains(&name)
            && let Some((text, ty)) = self.builtin_ident(name)
        {
            return Ok((text, ty));
        }
        if self.builtin_ident(name).is_some() {
            return Err(ShaderError::new(
                "unsupported",
                format!("assigns to the read-only shader input `{name}`"),
            ));
        }
        Err(unknown(name))
    }

    // --- functions and the entry point ---

    fn function(&mut self, func: &Func) -> Result<String, ShaderError> {
        let ret = hlsl_ty(&func.ret)?;
        let mut params = Vec::new();
        for (ty, name) in &func.params {
            params.push((hlsl_ty(ty)?, name.clone()));
        }
        self.user_fns.insert(
            func.name.clone(),
            (ret, params.iter().map(|(t, _)| *t).collect()),
        );

        let mut out = String::new();
        let sig: Vec<String> = params
            .iter()
            .map(|(ty, name)| format!("_p_{name}: {}", ty.wgsl()))
            .collect();
        let arrow = if ret == Ty::Void {
            String::new()
        } else {
            format!(" -> {}", ret.wgsl())
        };
        out.push_str(&format!(
            "fn m_{}({}){arrow} {{\n",
            func.name,
            sig.join(", ")
        ));
        self.scopes.push(HashMap::new());
        // HLSL parameters are mutable locals; WGSL parameters are not. Shadow
        // each into a `var`.
        for (ty, name) in &params {
            out.push_str(&format!("    var m_{name}: {} = _p_{name};\n", ty.wgsl()));
            self.declare(name, *ty);
        }
        self.current_ret = ret;
        self.stmts(&func.body, &mut out, 1)?;
        self.current_ret = Ty::Void;
        self.scopes.pop();
        // A body that can fall off the end still needs a return in WGSL.
        if ret != Ty::Void && !matches!(func.body.last(), Some(Stmt::Return(_))) {
            out.push_str(&format!("    return {};\n", ret.zero()));
        }
        out.push_str("}\n\n");
        Ok(out)
    }

    fn fs_main(&mut self, body: &[Stmt]) -> Result<String, ShaderError> {
        let mut out = String::new();
        match self.stage {
            Stage::Warp => out.push_str(
                "@fragment\n\
                 fn fs_main(@location(0) _in_uv: vec2<f32>, @location(1) _in_uv_orig: vec2<f32>) -> @location(0) vec4<f32> {\n\
                 \x20   var _lmv_uv: vec2<f32> = _in_uv;\n\
                 \x20   var _lmv_uv_orig: vec2<f32> = _in_uv_orig;\n",
            ),
            Stage::Comp => out.push_str(
                "@fragment\n\
                 fn fs_main(@location(0) _in_uv: vec2<f32>) -> @location(0) vec4<f32> {\n\
                 \x20   var _lmv_uv: vec2<f32> = _in_uv;\n\
                 \x20   var _lmv_uv_orig: vec2<f32> = _in_uv;\n",
            ),
        }
        // MilkDrop's `rad`/`ang` normalization — the longer axis reads 1, so the
        // pair matches what the EEL per-vertex program saw
        // (`MilkRuntime::run_vertex`). `U.aspect.zw` is that pair in both
        // orientations.
        out.push_str(
            "    let _lmv_p = (_lmv_uv_orig - vec2<f32>(0.5, 0.5)) * vec2<f32>(2.0, -2.0) * U.aspect.zw;\n\
             \x20   var _lmv_rad: f32 = length(_lmv_p);\n\
             \x20   var _lmv_ang: f32 = atan2(_lmv_p.y, _lmv_p.x);\n\
             \x20   var _lmv_ret: vec3<f32> = vec3<f32>(0.0);\n\
             \x20   var _lmv_hue: vec3<f32> = mix(\n\
             \x20       mix(U.hue[0].xyz, U.hue[1].xyz, _lmv_uv.x),\n\
             \x20       mix(U.hue[2].xyz, U.hue[3].xyz, _lmv_uv.x),\n\
             \x20       _lmv_uv.y);\n",
        );
        for (shadow, source) in std::mem::take(&mut self.shadow_inits) {
            out.push_str(&format!("    {shadow} = {source};\n"));
        }
        self.scopes.push(HashMap::new());
        self.current_ret = Ty::Void;
        self.stmts(body, &mut out, 1)?;
        self.scopes.pop();
        match self.stage {
            // The reference's target was 8-bit: feedback saturates at 1.0, which
            // is what keeps a `decay >= 1` preset bounded here too. The select
            // scrubs a NaN out of the feedback loop — comparisons with NaN are
            // false, so a poisoned lane resets to 0 instead of spreading.
            Stage::Warp => out.push_str(
                "    var _lmv_o = clamp(_lmv_ret, vec3<f32>(0.0), vec3<f32>(1.0));\n\
                 \x20   _lmv_o = select(vec3<f32>(0.0), _lmv_o, _lmv_o == _lmv_o);\n\
                 \x20   return vec4<f32>(_lmv_o, 1.0);\n}\n",
            ),
            // The composite goes to the screen: brightness scales the light,
            // occlude is the coverage the backdrop blend reads (ADR-0085).
            Stage::Comp => out.push_str(
                "    var _lmv_o = max(_lmv_ret, vec3<f32>(0.0));\n\
                 \x20   _lmv_o = select(vec3<f32>(0.0), _lmv_o, _lmv_o == _lmv_o);\n\
                 \x20   return vec4<f32>(_lmv_o * U.misc.y, U.misc.z);\n}\n",
            ),
        }
        Ok(out)
    }
}

fn unknown(name: &str) -> ShaderError {
    if name.starts_with("sampler_") || name.starts_with("texsize_") {
        return disk_texture(name.trim_start_matches("sampler_"));
    }
    ShaderError::new(
        "unknown-name",
        format!(
            "`{name}` is not a MilkDrop shader input, an intrinsic, or anything the preset defined"
        ),
    )
}

const LANES: &str = "xyzw";

fn lane_name(lane: u8) -> char {
    LANES
        .as_bytes()
        .get(lane as usize)
        .map_or('x', |b| *b as char)
}

/// Swizzle character → lane index, accepting both `xyzw` and `rgba`.
fn lane_of(c: char) -> Option<u8> {
    match c {
        'x' | 'r' => Some(0),
        'y' | 'g' => Some(1),
        'z' | 'b' => Some(2),
        'w' | 'a' => Some(3),
        _ => None,
    }
}

fn swizzle_lanes(member: &str, width: u8) -> Result<Vec<u8>, ShaderError> {
    if member.is_empty() || member.len() > 4 {
        return Err(ShaderError::new(
            "unsupported",
            format!("swizzle `.{member}`"),
        ));
    }
    member
        .chars()
        .map(|c| {
            lane_of(c).filter(|l| *l < width).ok_or_else(|| {
                ShaderError::new(
                    "unsupported",
                    format!("swizzle `.{member}` on a {width}-component value"),
                )
            })
        })
        .collect()
}

/// A member read: a swizzle, including HLSL's scalar broadcast (`x.xxx`).
fn swizzle(value: Val, member: &str) -> Result<Val, ShaderError> {
    let (text, ty) = value;
    let normalized: String = member
        .chars()
        .map(|c| lane_of(c).map_or(c, lane_name))
        .collect();
    match ty {
        Ty::Vec(n) => {
            let lanes = swizzle_lanes(member, n)?;
            let out_ty = if lanes.len() == 1 {
                Ty::F32
            } else {
                Ty::Vec(lanes.len() as u8)
            };
            Ok((format!("({text}).{normalized}"), out_ty))
        }
        Ty::BVec(n) => {
            let lanes = swizzle_lanes(member, n)?;
            let out_ty = if lanes.len() == 1 {
                Ty::Bool
            } else {
                Ty::BVec(lanes.len() as u8)
            };
            Ok((format!("({text}).{normalized}"), out_ty))
        }
        Ty::F32 => {
            // `scalar.xxx` broadcasts in HLSL.
            let lanes = swizzle_lanes(member, 1)?;
            if lanes.iter().any(|l| *l != 0) {
                return Err(ShaderError::new(
                    "unsupported",
                    format!("swizzle `.{member}` on a scalar"),
                ));
            }
            if lanes.len() == 1 {
                Ok((text, Ty::F32))
            } else {
                Ok((
                    format!("vec{}<f32>({text})", lanes.len()),
                    Ty::Vec(lanes.len() as u8),
                ))
            }
        }
        other => Err(ShaderError::new(
            "unsupported",
            format!("member `.{member}` on a {}", other.wgsl()),
        )),
    }
}

/// A WGSL float literal from an HLSL one — dots repaired, hex widened.
fn number(text: &str) -> Result<String, ShaderError> {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        let value = u64::from_str_radix(hex, 16)
            .map_err(|_| ShaderError::new("parse", format!("hex literal `{text}`")))?;
        return Ok(format!("{value}.0"));
    }
    // Split off an exponent, then repair the mantissa's dot.
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(at) => (text.get(..at).unwrap_or(text), text.get(at..).unwrap_or("")),
        None => (text, ""),
    };
    let mantissa = if mantissa.contains('.') {
        let mut m = mantissa.to_string();
        if m.starts_with('.') {
            m.insert(0, '0');
        }
        if m.ends_with('.') {
            m.push('0');
        }
        m
    } else {
        format!("{mantissa}.0")
    };
    if mantissa.parse::<f64>().is_err() {
        return Err(ShaderError::new(
            "parse",
            format!("numeric literal `{text}`"),
        ));
    }
    Ok(format!("{mantissa}{exponent}"))
}

/// An index expression: a literal stays an integer literal, anything else is
/// computed in f32 and floored into an i32.
fn index_text(expr: &str) -> String {
    let trimmed = expr.trim();
    if let Ok(n) = trimmed.trim_end_matches(".0").parse::<i64>() {
        return n.to_string();
    }
    format!("i32({trimmed})")
}
