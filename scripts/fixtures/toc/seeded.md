# The seeded document — every heading shape this corpus actually contains

The block below is **generated** and is the expected output: `--self-test` compares what
`scripts/toc.mjs` produces against these twelve committed rows, so a regression in the anchor
algorithm shows up as a diff here rather than as a plausible-looking anchor nobody re-checked.

<!-- toc:begin depth=3 -->
- [The `shot` CLI](#the-shot-cli)
  - [`--render`: a music video from a track](#--render-a-music-video-from-a-track)
  - [Seeded randomness — `hash` and `noise`](#seeded-randomness--hash-and-noise)
- [Coverage at 92 % of the suite](#coverage-at-92--of-the-suite)
  - [DX12 / Vulkan, and Metal](#dx12--vulkan-and-metal)
  - [The linked heading](#the-linked-heading)
- [A repeated heading](#a-repeated-heading)
  - [reaction_diffusion keeps its underscores](#reaction_diffusion-keeps-its-underscores)
- [A repeated heading](#a-repeated-heading-1)
  - [~~A struck heading~~ (Plan 0063)](#a-struck-heading-plan-0063)
  - [Something *moved* — the fixtures](#something-moved--the-fixtures)
- [Trailing section](#trailing-section)
<!-- toc:end -->

## The `shot` CLI

Backticks are punctuation and are deleted where they sit, so a backtick against a word adds no
hyphen.

### `--render`: a music video from a track

The first of the two anchors this repository already links. It pins backtick stripping and colon
removal together.

### Seeded randomness — `hash` and `noise`

The em-dash sits **between two spaces**, and deleting it leaves both — which is why the anchor
carries a doubled hyphen. This is the counter-intuitive half of the rule and the reason a
hand-written anchor is usually wrong.

## Coverage at 92 % of the suite

A `%` doubles the hyphen for the same reason the em-dash does.

### DX12 / Vulkan, and Metal

So does a `/`. The comma does not, because it sits against a word rather than between two spaces.

### The [linked heading](target.md)

A heading that is partly a link keeps its text and loses its target, in the anchor **and** in the
row — a row nesting a second link would not render at all.

## A repeated heading

The first occurrence takes the bare anchor.

### reaction_diffusion keeps its underscores

GitHub keeps `_`, so a bare snake_case identifier anchors as itself. A rule that stripped `_` as an
emphasis marker would break every heading in `presets/README.md`.

## A repeated heading

The second occurrence dedupes with `-1`. The two backlog files carry six repeated heading texts
between them and the archive eight, so this path is load-bearing rather than theoretical.

### ~~A struck heading~~ (Plan 0063)

`~~` and the parentheses are punctuation; the heading text keeps them and the anchor does not.

### Something *moved* — the fixtures

Emphasis markers need no special case for the same reason.

#### This heading is level 4 and must not appear

`depth=3` means levels 2 through 3. A level-4 heading is body, not a row.

## Trailing section

Last, so that the heading scan running to the end of the file is exercised.
