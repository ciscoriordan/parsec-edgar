# Parsec EDGAR website

Cloudflare Pages serves the static files in `dist/`. Preview locally from this directory with `python -m http.server --directory dist`.

The filing demo uses Apple Inc.'s fiscal 2026 Form 10-K, accession `0000320193-26-000026`. The JSON is generated with Parsec EDGAR 0.2.2.

```sh
parsec-edgar sections 320193 --accession 0000320193-26-000026 > dist/assets/filing.json
parsec-edgar tables 320193 --accession 0000320193-26-000026 --item 8 |
  jq '.tables[] | select(.rows[1][0] == "iPhone")' > dist/assets/table.json
```

The wordmark and favicon use the path from the repository's `assets/readme-dark.svg`. Cloudflare Pages deploys from the Parsec GitHub repository on pushes to `main`. The build output directory is `website/dist`, with no build command.
