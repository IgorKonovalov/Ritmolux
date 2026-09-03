# Two unpaired markers

A stray end marker, before any begin:

<!-- toc:end -->

...and a begin marker with no end after it anywhere in the file:

<!-- toc:begin depth=2 -->

## A heading that is never collected

Both are typos rather than drift, and the file is **left untouched** rather than rewritten. The
alternative — treating everything after an unclosed `toc:begin` as block body — would delete a
document on a missing marker, which is the one failure this parser must not have.
