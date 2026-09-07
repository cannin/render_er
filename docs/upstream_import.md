# Upstream renderer import

The Go, R, and Python renderer baselines were imported from
[`cannin/render_sbgn`](https://github.com/cannin/render_sbgn) at commit
`f0b98cfc92ad1db353aaf4aa495e488740b88c1d`.

This repository remains independent: it has no Git remote pointing to
`render_sbgn`, and changes made here are not pushed upstream.

The language-specific `.gitignore` files start from the matching templates in
[`github/gitignore`](https://github.com/github/gitignore), retrieved from its
`main` branch on 2026-09-07. Project-specific generated artifacts are listed at
the end of each template.
