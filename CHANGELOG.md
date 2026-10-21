# Changelog

## 0.1.0 (2026-10-22)


### ⚠ BREAKING CHANGES

* the public binaries are now ptero and pterobacter.

### Features

* add tero and terobacter commands and a saved-evidence v1 tracer ([c29e5cc](https://github.com/mbhall88/pterobacter/commit/c29e5cc48f7ae277476d546b95684c051a0f5491))
* **anim:** compare MUMmer4 with persistent resource limits ([a5bb4d9](https://github.com/mbhall88/pterobacter/commit/a5bb4d97ec3c0a58efe7add5a92b4091eb82c082))
* **anim:** measure reciprocal pairs with maintained MUMmer4 ([98acf42](https://github.com/mbhall88/pterobacter/commit/98acf42aef60f5bd6449adaf3d918bb779cb6415))
* **anim:** probe MUMmer4 maintenance and shared-process concurrency ([96202ea](https://github.com/mbhall88/pterobacter/commit/96202ea5e41b8ee07b457db0f75ed7a5094d4569))
* authenticate versioned reference package ([1720a35](https://github.com/mbhall88/pterobacter/commit/1720a35228d4f13630625b4940dd75d90453d744))
* bind the release candidate to the P4a evaluation interface ([#35](https://github.com/mbhall88/pterobacter/issues/35)) ([834f4c2](https://github.com/mbhall88/pterobacter/commit/834f4c288e9c163f316f3668f0219004d0adc6da)), closes [#17](https://github.com/mbhall88/pterobacter/issues/17)
* **cli:** accept assembly lists, sample sheets and sample names ([1416522](https://github.com/mbhall88/pterobacter/commit/141652283d9532f741864949db9b4cb4236677eb)), closes [#40](https://github.com/mbhall88/pterobacter/issues/40)
* **cli:** run batches concurrently and reuse verified ANIm pairs ([4dccd3d](https://github.com/mbhall88/pterobacter/commit/4dccd3d689f7dfc124cd9b577392a71dc0d00d4b)), closes [#16](https://github.com/mbhall88/pterobacter/issues/16)
* **database:** manage pinned references and optional typing ([7ff49bf](https://github.com/mbhall88/pterobacter/commit/7ff49bf9f5cb9335cef59b3fef2a1e6e8bf89610))
* **database:** pin published reference version 2026-10-15 ([1fad912](https://github.com/mbhall88/pterobacter/commit/1fad912bd1c788ae6b5221ca8716eef2a862baf8))
* **decision:** reproduce frozen v1 species and subspecies decisions ([ffa2818](https://github.com/mbhall88/pterobacter/commit/ffa2818a9a89b32fdcca114b14c4cf5258ad57cb))
* default to boundary ANIm and simplify public options ([#45](https://github.com/mbhall88/pterobacter/issues/45)) ([0060527](https://github.com/mbhall88/pterobacter/commit/00605278863309bc42cf03a6b5b2a30767382015))
* enable MLST by default and simplify the user guide ([2ec46e3](https://github.com/mbhall88/pterobacter/commit/2ec46e32c24d8f50689226cffaddca0caa915b3d))
* **genomovar:** add development-only masking diagnostics ([5a22848](https://github.com/mbhall88/pterobacter/commit/5a22848d1c8f76ade5474dc4c5d085d5e908e6ed)), closes [#14](https://github.com/mbhall88/pterobacter/issues/14)
* **genomovar:** report authenticated all-member assignments ([46282f5](https://github.com/mbhall88/pterobacter/commit/46282f5aa06ab6259c2b56f0a0464bb2cd6afaf8))
* make ANIm execution limits opt-in ([0e32774](https://github.com/mbhall88/pterobacter/commit/0e32774212ca5671e061c26e01d072987771a83f))
* **mlst:** add candidate runner and comparison harness, calibrate X-drop ([1ff5896](https://github.com/mbhall88/pterobacter/commit/1ff589628143e70b10e1ac5bfd92f23a85786526))
* **mlst:** add fixture generator, comparator runner and probe crate ([0fcfe78](https://github.com/mbhall88/pterobacter/commit/0fcfe781d45994a51d57d2a61a4b5e4c6524426f))
* **mlst:** measure all candidate routes and record results ([3188d96](https://github.com/mbhall88/pterobacter/commit/3188d96686861d4cc577c6b281f5bdacb3a9f986))
* **mlst:** report optional historical sequence typing ([fdfc825](https://github.com/mbhall88/pterobacter/commit/fdfc825d8b9b92614609f419f63c03c6afb2105d))
* **reference:** authenticate the versioned reference package ([9d19630](https://github.com/mbhall88/pterobacter/commit/9d19630712d2bece5f09d9f35f7c3ec26dc938e9))
* **reference:** verify component indexes and expose roles and scope ([f931d69](https://github.com/mbhall88/pterobacter/commit/f931d69a5fde08c8778bbed5dc5bc64e6d7f8c78))
* **report:** add TSV and Markdown reports and usage guide ([aa53a8e](https://github.com/mbhall88/pterobacter/commit/aa53a8ec9f311ae334f36d7668f00da319518cd3)), closes [#15](https://github.com/mbhall88/pterobacter/issues/15)
* **rescue:** decide v2 rescue from saved evidence with a dry rescue plan ([585e4a3](https://github.com/mbhall88/pterobacter/commit/585e4a3edb09249c78b5eb1773eecb1de369e6f9)), closes [#9](https://github.com/mbhall88/pterobacter/issues/9)
* **skani:** embed pinned engine with portable AVX2 compatibility ([56b5955](https://github.com/mbhall88/pterobacter/commit/56b59552b3fde76580eec9a882553253abd4ffd2))
* **thresholds:** support custom v2 ANI settings ([bde093b](https://github.com/mbhall88/pterobacter/commit/bde093ba21bd1fca5ab276a504c7c78d6e7acfff))
* **workflow:** classify whole assemblies under v1 ([4f5e2de](https://github.com/mbhall88/pterobacter/commit/4f5e2def9ec88d1a14a5b5909e81d4c58a00f943)), closes [#7](https://github.com/mbhall88/pterobacter/issues/7)
* **workflow:** run native v2 ANIm rescue ([aa83775](https://github.com/mbhall88/pterobacter/commit/aa837756a5fa0a16fa3fd913248bf8c4d4335707))


### Bug Fixes

* **anim:** charge failed cleanup overruns before further admission ([1220beb](https://github.com/mbhall88/pterobacter/commit/1220beb9becd39c6e5d396f5a26ced3533c92c64))
* **anim:** contain timed workers and retain failed comparisons ([96f168e](https://github.com/mbhall88/pterobacter/commit/96f168ec9fd9070ff3450751cda37384c95ce5b8))
* **anim:** make filter tie handling portable ([bf5d877](https://github.com/mbhall88/pterobacter/commit/bf5d877c9569ea90acbd75b2ac7ea282195f7caa))
* **anim:** monitor native peak RSS on macOS ([8b7c56d](https://github.com/mbhall88/pterobacter/commit/8b7c56d837ffb9fa1e57fc4ae3317d1dcd002596))
* **anim:** normalize native FASTA formatting without changing bases ([62cb48f](https://github.com/mbhall88/pterobacter/commit/62cb48f49dca3f954a8f25404f8615ba1bd4c69b))
* **anim:** preserve exact JSON worker time limits ([e82d690](https://github.com/mbhall88/pterobacter/commit/e82d690056962291afc453ef88ebee653f829cc4))
* **anim:** stop candidate admission after native failure ([1c25b24](https://github.com/mbhall88/pterobacter/commit/1c25b242cdd9bddf9706287e12200af07d2e9560))
* **anim:** validate saved context and delta record headers ([a65f09b](https://github.com/mbhall88/pterobacter/commit/a65f09b1e9f605e1ab3e5fb23271aa02083f6400))
* **anim:** write portable LF endings in synthetic result tables ([cb79e25](https://github.com/mbhall88/pterobacter/commit/cb79e25e98643ff89736bb7ffbd7811c9e3d642d))
* **build:** discover Homebrew GNU C++ and preserve vendored measurements ([#38](https://github.com/mbhall88/pterobacter/issues/38)) ([eae4bc6](https://github.com/mbhall88/pterobacter/commit/eae4bc6d15d6f0f96d5fff273bac805b3d255034))
* **cli:** support real-genome examples and command-name option values ([49a0a2d](https://github.com/mbhall88/pterobacter/commit/49a0a2d557e7e995a8a5364acafa694fdfbeebda))
* **evidence:** reject unmapped assignable references ([6ab918b](https://github.com/mbhall88/pterobacter/commit/6ab918b4a9af98c3722b349ecfb2b714f8fd0651))
* match frozen reference validation and flag unapplied v1 checks ([adde942](https://github.com/mbhall88/pterobacter/commit/adde9424a0fdc03f8a5dd510df0c1fee656e3a58))
* **mlst:** act on code review, show state and candidate ST per fixture ([e355f62](https://github.com/mbhall88/pterobacter/commit/e355f6253a8f6e294d1328de5a0193c184bfcb64))
* **mlst:** enforce stop rules, retain diagnostics, check real importer ([24d0ff2](https://github.com/mbhall88/pterobacter/commit/24d0ff2368f4d81cc814ca50448bd8d0c17ad5d1))
* **mlst:** share CPU budget across drivers, safe launch, stop crashed perf runs ([f8eacda](https://github.com/mbhall88/pterobacter/commit/f8eacda3e4757b3a377f69a023fdd694a7c502a8))
* **reference:** authenticate the frozen package model ([8434f55](https://github.com/mbhall88/pterobacter/commit/8434f55e7052809c69358f0b7ee6b3255a2788c2))
* **reference:** require the expected manifest digest and refuse symlinks ([7dfce0e](https://github.com/mbhall88/pterobacter/commit/7dfce0e3d421a4b94bab7f996c8bcc1483609c42))
* reject percentages with more than two decimals instead of rounding ([cea638b](https://github.com/mbhall88/pterobacter/commit/cea638b94fe160f7c50f1836f7c68bbb4e1d3e92))
* **skani:** make orientation ties independent of filenames ([22a377a](https://github.com/mbhall88/pterobacter/commit/22a377a68ae532d1c05f4eafac22025b85d55755))


### Code Refactoring

* rename the package and commands to Pterobacter ([#42](https://github.com/mbhall88/pterobacter/issues/42)) ([5a588ec](https://github.com/mbhall88/pterobacter/commit/5a588ec8e2ba3def587c44dd05191c1c3bce0365))
