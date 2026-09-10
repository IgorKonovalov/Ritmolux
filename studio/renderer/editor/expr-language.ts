/**
 * Highlighting for a preset file.
 *
 * **What is highlighted here is structure, not vocabulary.** Table headers,
 * keys, quoted expressions, numbers and comments are TOML's own shape and are
 * the same whatever the engine's grammar contains, so colouring them invents
 * nothing. The grammar's *names* — its functions and its variables — are
 * deliberately **not** coloured, because this studio has no honest list of
 * them: the engine declares them (`VAR_NAMES` and the function match in
 * `core/src/preset/expr.rs`) and `--schema` does not export them, and a list
 * typed in here would be a private catalogue that goes stale the first time the
 * grammar gains a function, silently and invisibly (ADR-0170).
 *
 * `grammar` is the seam that closes the gap. Hand it the roster the day the
 * schema carries one and every name in it is highlighted; hand it nothing, as
 * the studio does today, and no identifier is coloured at all. There is no
 * third setting in which some names are known and others are not — that is the
 * state that misleads an author into thinking a real function is a typo.
 */
import { StreamLanguage, type StringStream } from '@codemirror/language'
import { tags } from '@lezer/highlight'

/** The engine's expression vocabulary, when something can say what it is. */
export interface Grammar {
  functions: readonly string[]
  variables: readonly string[]
}

interface State {
  /** Inside a quoted value, which is where an expression lives. */
  quote: string | undefined
}

const KEY = /[A-Za-z_][A-Za-z0-9_-]*/
const NUMBER = /[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?/

export function presetLanguage(grammar?: Grammar): StreamLanguage<State> {
  const functions = new Set(grammar?.functions ?? [])
  const variables = new Set(grammar?.variables ?? [])

  return StreamLanguage.define<State>({
    name: 'ritmolux-preset',
    startState: () => ({ quote: undefined }),

    // `function` is a **modifier** in the tag system and not a tag of its own,
    // so a token named for it cannot be resolved and the mode fails to load.
    // These two names are this mode's own, mapped to real tags here.
    tokenTable: {
      grammarFunction: tags.function(tags.variableName),
      grammarVariable: tags.variableName,
    },

    token(stream: StringStream, state: State): string | null {
      if (state.quote !== undefined) {
        // Inside an expression. Numbers and operators are the grammar-neutral
        // half; an identifier is coloured only when the roster says what it is.
        if (stream.eat(state.quote) !== undefined) {
          state.quote = undefined
          return 'string'
        }
        if (stream.match(NUMBER) !== null) return 'number'
        // `match` answers `RegExpMatchArray | null` for a pattern and `boolean`
        // for a string, and the union is what the signature carries; a regex
        // never yields the boolean arm.
        const word = stream.match(KEY)
        if (typeof word !== 'boolean' && word !== null) {
          const name = word[0]
          if (functions.has(name)) return 'grammarFunction'
          if (variables.has(name)) return 'grammarVariable'
          return 'string'
        }
        if (stream.match(/[+\-*/%<>=!&|?:,()]+/) !== null) return 'operator'
        stream.next()
        return 'string'
      }

      if (stream.sol() && stream.match(/\s*\[[^\]]*\]/) !== null) return 'heading'
      if (stream.eatSpace()) return null
      if (stream.peek() === '#') {
        stream.skipToEnd()
        return 'comment'
      }
      const quote = stream.peek()
      if (quote === '"' || quote === "'") {
        stream.next()
        state.quote = quote
        return 'string'
      }
      if (stream.match(NUMBER) !== null) return 'number'
      if (stream.match(KEY) !== null) return 'propertyName'
      stream.next()
      return null
    },
  })
}
