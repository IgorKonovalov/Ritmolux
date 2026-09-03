# Markers and headings inside a fence

<!-- toc:begin depth=3 -->
- [A real heading](#a-real-heading)
<!-- toc:end -->

## A real heading

The block above holds exactly one row. Everything in the fence below is a document *describing*
this syntax, which is not a document carrying a block:

```markdown
<!-- toc:begin depth=2 -->
- [Not a row](#not-a-row)
<!-- toc:end -->

## A fenced heading, which is not a heading
```

Plan 0151's own `## Data shapes` section is the first real document of that shape, and a parser
that read it would have rewritten the plan's worked example.
