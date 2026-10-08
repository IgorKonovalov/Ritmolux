# ADR-0269 — A solid voxel is lit by one key light and its own occlusion, and its light blooms like any other

> **Status:** proposed 2026-10-08
> **Date:** 2026-10-08
> **Related plan(s):** [0256](../plans/0256-the-voxels-turn-solid.md); builds on [ADR-0268](0268-a-3d-automaton-is-a-voxel-system-marched-as-an-emitting-absorbing-volume.md)

## Context

Every look this engine draws is emissive. Backlog 0092 records that nothing computes a surface
normal or evaluates a light, and that the whole downstream chain assumes emission. ADR-0046's linear
light is built for additive accumulation, bloom's bright-pass reads emitted light, and ADR-0201's
alpha is coverage. The same entry leaves one question open for whoever lights something first:
**does a lit highlight bloom?** It is not an emitter, and it may be the brightest thing on screen.

The owner wants the 3D automaton of ADR-0268 drawn as solid cubes. Unlit solid cubes are a flat
silhouette: a cluster of same-coloured faces with no edges between them. What makes voxel automata
readable is the shading that separates the faces and darkens the creases. So this look cannot stay
emissive, and it is the engine's first non-emissive register.

Three facts narrow the design:

- **ADR-0268's march already finds the surface.** The DDA walks the ray cell by cell. Stopping at the
  first live cell gives the hit, and the axis the last step crossed gives the face normal exactly.
  That is one of six normals, with no gradient and no depth buffer.
- **Voxel ambient occlusion is exact and local.** The occlusion at each corner of a face is a
  function of the three cells around that corner in the layer in front of the face. Interpolated
  across the face, it darkens every crease and inner corner. That is what makes a voxel structure
  read as structure.
- **The camera orbits.** A light fixed in world space puts the lit side behind the volume for half
  of an orbit.

## Decision

We will give `voxel` a structural `present = "glow" | "solid"`, with `glow` the default and
byte-identical to ADR-0268. **`solid` is the same march with a terminating surface.** The ray
accumulates the emission and absorption of decay-stage cells exactly as `glow` does, and stops at the
first live cell. It shades that cell and composites the shade under the light gathered so far:

```
albedo = palette(age coordinate) * brightness * shell(cell)
shade  = albedo * (ambient * ao + key * max(0, dot(n, l)))
C     += T * shade;  T = 0
```

- **The normal** is the face the ray entered, from the DDA's last stepped axis.
- **The key light** is one directional light, given in **camera space** by `light_yaw` and
  `light_pitch`, so the lit side stays lit through an orbit. `key` and `ambient` are its two
  strengths. All four are modal and bindable.
- **The occlusion** is the per-corner voxel AO of the four corners of the hit face, interpolated
  bilinearly at the hit point and mixed in by `ao` (0 = none).
- **No shadows and no specular.** Each would need a second ray or a view-dependent term. Both are
  followups that this decision does not preclude.

**A lit surface enters the composite as linear light like everything else, and bloom treats it the
same.** No flag marks it as reflected, and the bright-pass reads it as it reads every scene. That
answers backlog 0092's open question for this system. In this engine brightness is authored: an
author who wants the faces to bloom raises `brightness` or `key` past the threshold, and one who
does not leaves them under it. A lit pixel has `alpha = occlude`, because the surface covers it
fully.

This ADR lights `voxel` only. It does not establish a light rig for the engine, and backlog 0092's
own ask, a matcap on the shape field, stays its own decision.

## Consequences

### Positive
- The faces, creases and corners of a structure read, which is what a voxel automaton needs and
  what unlit solid cubes cannot show.
- No depth buffer, mesh, normal map or extra pass. The occlusion is exact rather than a screen-space
  estimate, and a solid march costs no more than a glow march, usually less, because it stops at the
  first hit.
- The decay-stage haze in front of the surface survives, so `solid` can read as a lit core inside
  its own glowing debris.
- Backlog 0092's bloom question has an answer that the next lit register can follow or overturn
  explicitly.

### Negative
- **Darkness becomes shape information**, and the chain does not know it. A shadowed face is dark
  because of the light, not because nothing is there, but bloom, trails and the post stages read it
  as low emission. A trail on a solid preset smears shading as if it were light.
- **Blooming lit surfaces is a judgement call.** A preset can bloom its faces into a glow that reads
  emissive again. That is the author's lever, and nothing stops an author from undoing the shading
  by accident.
- **A camera-space light does not stay fixed on the world.** As the camera orbits, the lit side
  turns with it, so the structure looks lit from the viewer's side rather than lit in a place. That
  is the right default for an orbiting show. A world-fixed light would need another mode.
- **Faces show the grid.** A solid voxel is a cube, and a 64³ grid on `Floor` draws visibly larger
  cubes than 128³ on `Rich`. That is the look, and it makes the content cap more visible than it is
  on `glow`.

## Alternatives considered

### Alternative A — instanced cube meshes with a depth buffer
Draw each live cell's visible faces as geometry and depth-test them. Rejected because it brings the
engine's first depth attachment, needs a GPU face-extraction pass over up to 2 M cells to stay
inside a vertex budget, still needs a separate AO computation, and draws nothing ADR-0268's march
cannot already find for free.

### Alternative B — a matcap
Look the face normal up in a small sphere image, as backlog 0092 proposed for the shape field.
Rejected for voxels because a voxel has six normals, so a matcap reduces to six fixed colours. That
gives no light direction to bind, and the AO, which is what makes the structure read, is still
needed on top.

### Alternative C — exempt lit surfaces from bloom
Write lit light to a channel bloom's bright-pass ignores. Rejected because it adds a second kind of
light to a chain built on one, and every post stage would then have to choose which kind it reads.
Brightness is already the author's: an author can keep a lit preset out of bloom with the levers
that already exist.
