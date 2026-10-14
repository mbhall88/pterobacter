# Build the documentation site

The guide uses [mdBook](https://rust-lang.github.io/mdBook/) 0.5.4. Install that version, then run from the repository root:

```sh
cargo install mdbook --version 0.5.4 --locked
python3 scripts/build_docs.py
python3 -m http.server --directory target/docs 8000
```

Open `http://localhost:8000`. The site has search, a chapter sidebar and light/dark themes. Edit the Markdown files under `docs/`; `docs/SUMMARY.md` sets the main navigation. The builder follows local guide links and includes linked reference pages and assets. Source-code links lead back to GitHub.

Pull requests build a `documentation-preview` artifact in the Documentation workflow. A maintainer can later enable GitHub Pages with GitHub Actions as its source and run that workflow on `main` with `publish` selected. Publication is manual; ordinary pushes and pull requests only build the preview. Keep the current release hold until author approval, and check Pages visibility before deploying a private repository's guide.

When a public site is available, update the README documentation badge and Cargo metadata to its actual URL. Add package-version and download badges only when those distributions exist.
