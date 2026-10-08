# Changelog

## [0.13.1](https://github.com/Wpnx330/spark-dashboard/compare/spark-dashboard-v0.13.0...spark-dashboard-v0.13.1) (2026-10-08)


### Dependencies & Chores

* **ci:** publish container image to our own GHCR namespace ([324d39b](https://github.com/Wpnx330/spark-dashboard/commit/324d39b6689eceeed5265ab9cb15ea4f3f36b210))

## [0.13.0](https://github.com/Wpnx330/spark-dashboard/compare/spark-dashboard-v0.12.0...spark-dashboard-v0.13.0) (2026-10-08)


### Features

* add cumulative token totals to engine throughput cards ([dd80c8e](https://github.com/Wpnx330/spark-dashboard/commit/dd80c8efe7664bb4755631393c447ae269064e2a))
* add historical dashboard with dual Metrics|Historical toggle ([e4cb1fa](https://github.com/Wpnx330/spark-dashboard/commit/e4cb1fa662880a2735b1c5d0402d967dd8d444b1))
* add TAR (Token Acceptance Rate) as yellow line on Cache chart ([4dcfda2](https://github.com/Wpnx330/spark-dashboard/commit/4dcfda2ef44c362bdf7b990d2954cd26bd7ab3f6))
* add Time Per Output Token (TPOT) to engine latency card ([20bc677](https://github.com/Wpnx330/spark-dashboard/commit/20bc6772ce70320b330dd28656c4f87a11cb21e5))
* bracket chart units in titles and clean up hover tooltips ([d92a975](https://github.com/Wpnx330/spark-dashboard/commit/d92a975f8537a3aed448410c4c1c0a7ed8ea89f6))
* cache resolved model info and support per-endpoint engine API keys ([18fa2f8](https://github.com/Wpnx330/spark-dashboard/commit/18fa2f8ff53c6d9d60577e9a375e28b23d78397b))
* **charts:** add per-chart time-scale cycling button (1m/5m/1h/24h) ([1f2424d](https://github.com/Wpnx330/spark-dashboard/commit/1f2424df1feb9c27d5ef0eca9e816dc133897de5))
* **dashboard:** sync GPU selection with active engine ([34387d2](https://github.com/Wpnx330/spark-dashboard/commit/34387d250a2bfabd03add9ae56213ab5c2437776))
* **dev:** add docker-dev.sh container test harness ([b6bbdca](https://github.com/Wpnx330/spark-dashboard/commit/b6bbdca008028de66bde77ecc6faed43c9cd0644))
* **docker:** add hardened multi-stage image and compose deployment ([6d0d700](https://github.com/Wpnx330/spark-dashboard/commit/6d0d700f72c4a5530062a2f0bc25873b6b814d7c))
* **docker:** harden runtime image with distroless base and self-probe healthcheck ([1ccc1db](https://github.com/Wpnx330/spark-dashboard/commit/1ccc1dbce75147a2667ecfc40549250d5d122fff))
* **engines:** associate detected engines with the GPUs they run on ([e406a50](https://github.com/Wpnx330/spark-dashboard/commit/e406a505f52a606a1b6a0138d8e49cc81e660fc0))
* **engines:** parse vLLM speculative-decoding metrics ([ac512e9](https://github.com/Wpnx330/spark-dashboard/commit/ac512e9bca984b846834c43dc59949f07b513ae7))
* **frontend:** adapt hardware cards to vertical space ([68ee1ff](https://github.com/Wpnx330/spark-dashboard/commit/68ee1fffa58cafb9ad1dcb5c0d58b9e46dee1451))
* **frontend:** add collapsible log viewer with WebSocket streaming ([d92e7c8](https://github.com/Wpnx330/spark-dashboard/commit/d92e7c8b8f3b636e0b011d034c27afe86a05eef8))
* **frontend:** add FlipCard with card-flip charts for engine metrics ([666f5dd](https://github.com/Wpnx330/spark-dashboard/commit/666f5dda2fe01504775eb4a430ecb43b5dc4064d))
* **frontend:** collapse chart row when console opens, not hardware ([e008cac](https://github.com/Wpnx330/spark-dashboard/commit/e008cac33c8e23eca89886a081f3bf16547f2891))
* **frontend:** enable mobile scroll with responsive viewport layout ([31d9f98](https://github.com/Wpnx330/spark-dashboard/commit/31d9f982420b45dd913923aa3a7e7c2904ee17cc))
* **frontend:** show per-engine GPU badge on multi-GPU hosts ([180e414](https://github.com/Wpnx330/spark-dashboard/commit/180e414f39550b5b1ace70918e0f749add6a7dd6)), closes [#45](https://github.com/Wpnx330/spark-dashboard/issues/45)
* **frontend:** surface speculative-decoding metrics in cache card ([feb936a](https://github.com/Wpnx330/spark-dashboard/commit/feb936a1c78a71c6024b8570e0dcef2171194293))
* **gpu:** add --simulate-gpus flag appending fictive GPUs to snapshots ([f6e29c5](https://github.com/Wpnx330/spark-dashboard/commit/f6e29c5b49ebb9178f3b03031aba4e95a0b02b32))
* **gpu:** make main Dashboard hardware panels switch between GPUs ([a34c31a](https://github.com/Wpnx330/spark-dashboard/commit/a34c31ab718bdd88d1cc4d955e71cecc00024670)), closes [#44](https://github.com/Wpnx330/spark-dashboard/issues/44)
* **gpu:** monitor all NVIDIA GPUs while preserving backward compatibility ([8d6033c](https://github.com/Wpnx330/spark-dashboard/commit/8d6033c3f94b6c00e50922aa1dbd423642441942))
* hide tooltip header on prefill and decode throughput charts too ([952d5ee](https://github.com/Wpnx330/spark-dashboard/commit/952d5ee85f680045217aefaf49bf9bd8baa57e65))
* **history:** add KV cache peak/avg and preemption tracking ([0c95782](https://github.com/Wpnx330/spark-dashboard/commit/0c95782e688868c0d4b542f8c125d608e6f6209a))
* **history:** add timeseries query endpoint for per-chart time scales ([9e794c7](https://github.com/Wpnx330/spark-dashboard/commit/9e794c726502497caabf3f3070a97ab9a8119d05))
* improvind dashboard and fixing data visibility ([#20](https://github.com/Wpnx330/spark-dashboard/issues/20)) ([c7851bd](https://github.com/Wpnx330/spark-dashboard/commit/c7851bd496f4d8c12b2cda163705e583b44fce6a))
* **logs:** container log streaming via bollard Docker API ([147f9c1](https://github.com/Wpnx330/spark-dashboard/commit/147f9c1bf221cae276762a6a893d6ee61798810d))
* make dashboard hardware- and host-agnostic ([3b77d5a](https://github.com/Wpnx330/spark-dashboard/commit/3b77d5a4ee95bdfd316f56a256a34ee06e339bda))
* make dashboard hardware- and host-agnostic ([9f0e3a6](https://github.com/Wpnx330/spark-dashboard/commit/9f0e3a6d2b321f42736a1ac03551da9b87072da2))
* model detail tags ([#18](https://github.com/Wpnx330/spark-dashboard/issues/18)) ([c0633bc](https://github.com/Wpnx330/spark-dashboard/commit/c0633bc9fe8d98499713084899b781773d38d99b))
* move tok/s into engine throughput titles and fix hover header ([bc2292f](https://github.com/Wpnx330/spark-dashboard/commit/bc2292f4eb6315534dbe48078fd6342ed0dd9c33))
* multi-engine dashboard with auto-rotation and live vLLM metrics ([#12](https://github.com/Wpnx330/spark-dashboard/issues/12)) ([53b2af8](https://github.com/Wpnx330/spark-dashboard/commit/53b2af81b0a21f2779bfac26e5a6bc2546d8490e))
* multi-node hardware monitoring with drill-down UI ([d862a3c](https://github.com/Wpnx330/spark-dashboard/commit/d862a3cf2c3267949bf9f309741d41f62139a7d2))
* **multi-node:** add node agent mode and node polling backend ([7db5a36](https://github.com/Wpnx330/spark-dashboard/commit/7db5a36fc29ac1567371a231fa99aedfcafb67b2))
* package and distribute via cargo install + systemd service ([b2a87d8](https://github.com/Wpnx330/spark-dashboard/commit/b2a87d8d42f29bd105c456bed581e657d6edafa0))
* persist queue_time_ms/tpot_ms, show all series at 1h/24h ([8d86684](https://github.com/Wpnx330/spark-dashboard/commit/8d86684befcda788466ac8feef7a15c66bfa28f9))
* plot prefix cache hit rate alongside KV cache over time ([4a105ca](https://github.com/Wpnx330/spark-dashboard/commit/4a105cae7dc1aaabdd2a090f5cf8bc53f51476af))
* record actual cluster power, backfill historical data, deprecate tp_gpu_count multiplier ([bddee09](https://github.com/Wpnx330/spark-dashboard/commit/bddee09c12b80dcd1b903bd13a529f1226a7c7fa))
* redesign node overview cards with gauges, fill container height ([34ea9bb](https://github.com/Wpnx330/spark-dashboard/commit/34ea9bb108f8c3f73948ada07a381dd5d2165d28))
* select physical/Wi-Fi network interface instead of loopback ([d7cdfbf](https://github.com/Wpnx330/spark-dashboard/commit/d7cdfbfc38b6cd10d49f976e1ecd4cc711abd0bb))
* **server:** add /healthz liveness endpoint ([d7bc7f7](https://github.com/Wpnx330/spark-dashboard/commit/d7bc7f7f95c8cf1f72e0c9ed12510707597bc187))
* slo goodput customization ([#23](https://github.com/Wpnx330/spark-dashboard/issues/23)) ([1576e43](https://github.com/Wpnx330/spark-dashboard/commit/1576e43f83460d3dfefc7145086a22b289dd8ac9))
* surface cumulative prefix cache queries on engine cache card ([0860324](https://github.com/Wpnx330/spark-dashboard/commit/08603248ddb94308d8ea4a24a83ace4cb6ab5051))
* surface engine deployment mode (Docker vs Direct) in tabs ([6e9de44](https://github.com/Wpnx330/spark-dashboard/commit/6e9de441bc13b0d451bbb47808373e3d8926b6d0))
* **vllm:** expand vLLM observability with latency percentiles, SLO goodput, and dashboard polish ([082bd17](https://github.com/Wpnx330/spark-dashboard/commit/082bd17adcaf4da6dc577ce1cefcd91b90464cae))


### Bug Fixes

* 24h chart x-axis — consistent labels with explicit time domain ([554c294](https://github.com/Wpnx330/spark-dashboard/commit/554c294d94fa6d8f06b44f6b3d5b9d3021533054))
* 24h chart x-axis garbage + data scrunched to right ([2a805d9](https://github.com/Wpnx330/spark-dashboard/commit/2a805d92e844ccfacb86ebb9c7fa9d37c40c083b))
* 24h charts miss recent hours when rollup fails + WAL TRUNCATE ([6825840](https://github.com/Wpnx330/spark-dashboard/commit/682584041dfadfcce3c9d13b2bf005bdd0b0257d))
* aggregate gauge metrics with MAX in 1h timespan to eliminate saw-tooth ([08377ab](https://github.com/Wpnx330/spark-dashboard/commit/08377abbf58db6682b70b0a4cf8f71451673f860))
* **ci:** gate crates.io publish via job env, add fmt+clippy components to toolchain pin ([475b7ee](https://github.com/Wpnx330/spark-dashboard/commit/475b7ee53866bdca16add77f8b46c5718ec071bb))
* **ci:** pin rust toolchain to deployed 1.96.1, guard crates.io publish on fork ([a527485](https://github.com/Wpnx330/spark-dashboard/commit/a5274856171da784f34370559a8e5d4a3d16cab2))
* compute vLLM prefix cache hit rate from counters ([#14](https://github.com/Wpnx330/spark-dashboard/issues/14)) ([1797e8a](https://github.com/Wpnx330/spark-dashboard/commit/1797e8a1b8f8208176771c727cd3d2a98dac4e10))
* **deps:** refresh Cargo.lock to latest compatible crate versions ([e2daca5](https://github.com/Wpnx330/spark-dashboard/commit/e2daca5b86cb591a32c0a3c390644dd72ed9a4f0))
* **dev:** prevent local tilde expansion of SPARK_DIR ([e1e8350](https://github.com/Wpnx330/spark-dashboard/commit/e1e8350d499c5b6c12470dee536d5d1c3be0b10b))
* downsample history data across full time range instead of truncating to last 60 points ([3261733](https://github.com/Wpnx330/spark-dashboard/commit/326173324fa2cd19fbbafc70e7724d6f8e5f63d8))
* **engines:** clear stale engine PIDs and test the detection merge ([6970153](https://github.com/Wpnx330/spark-dashboard/commit/69701531a5217ba2ba2fe1c7cd4d2d2c5baf0e37))
* **frontend:** log viewer compresses engine section, not hardware ([8d96761](https://github.com/Wpnx330/spark-dashboard/commit/8d9676141a7c645b130aafa94b3df5b6b7d5c512))
* **frontend:** scale GPU power gauge by observed peak when no cap ([e2d525e](https://github.com/Wpnx330/spark-dashboard/commit/e2d525eddc7a6becd55bccab5d9179986c6b4bcc))
* **gpu:** keep Dashboard hook order stable across the first snapshot ([2afad09](https://github.com/Wpnx330/spark-dashboard/commit/2afad09e8b3be91c7c4a31bc942f8e59f0c45c11))
* historical peak KV cache showing wrong values ([1884a21](https://github.com/Wpnx330/spark-dashboard/commit/1884a213d66566555a5c334db0c38ab592e83a5b))
* **history:** include bucket straddling range-start in BOTH agg paths ([6197dd9](https://github.com/Wpnx330/spark-dashboard/commit/6197dd9d307d9c71bc2db256bf7a63253912dcfa))
* **history:** local-day rollup + legacy-NULL aggregates ([06a25a3](https://github.com/Wpnx330/spark-dashboard/commit/06a25a3591540a5090e7af050b2a557ce0a65bad))
* **history:** re-aggregate current shifted-day bucket up to now in 1h-&gt;1d rollup ([0984ec1](https://github.com/Wpnx330/spark-dashboard/commit/0984ec1b1223d476be88502e8a580c67eb24eb40))
* **install:** refuse sudo invocation; let binary self-escalate ([942c796](https://github.com/Wpnx330/spark-dashboard/commit/942c7961b48fc01771ec5a34af659c4e8a2ddd5e))
* **install:** refuse sudo invocation; let binary self-escalate ([e778623](https://github.com/Wpnx330/spark-dashboard/commit/e77862341ddd62581e643643977c6cc9aa473bbe))
* KV cache peak display precision + comprehensive peak pipeline tests ([2ec36d5](https://github.com/Wpnx330/spark-dashboard/commit/2ec36d5f89bb692fd5492050ca92a53175e426b6))
* **logs:** address maintainer feedback on log streaming ([be66302](https://github.com/Wpnx330/spark-dashboard/commit/be663024996958339e9efb4b707f6e83effc7128))
* **logs:** follow engine container across swaps + supervisor retry ([dfda989](https://github.com/Wpnx330/spark-dashboard/commit/dfda98982d3743199a58fe7f29f52e825d79adc6))
* **metrics:** resolve GPU power limit via NVML fallback chain ([e68d7cb](https://github.com/Wpnx330/spark-dashboard/commit/e68d7cb2725d1f69f4a6648fe8738943ad24e776))
* **metrics:** stop double-counting local GPU in recorded cluster power ([c4cad42](https://github.com/Wpnx330/spark-dashboard/commit/c4cad42201c3c2b82da9aba05d74eac9f42c6826))
* node card gauges scale to container, prevent mobile clipping ([fa5f15c](https://github.com/Wpnx330/spark-dashboard/commit/fa5f15c76a0da0e349580a9a644a62550a9ce5e5))
* populate latency metrics in rollups and fix 1m/5m buffer maxPoints ([e6b03fa](https://github.com/Wpnx330/spark-dashboard/commit/e6b03fad3b3e9604ce4cb9841b89ef3fe98f9acd))
* query both 1h and 1s tables for ≤1h ranges ([af6a691](https://github.com/Wpnx330/spark-dashboard/commit/af6a691edff1d5e65a3d321ec447593718ebf92c))
* summary query misses current day + preemptions should be delta ([c4eef61](https://github.com/Wpnx330/spark-dashboard/commit/c4eef6114632eaf13a32e0a6ec80697cdd42155a))
* **test:** align MemoryCard test selectors with current StackedBar markup ([e499da9](https://github.com/Wpnx330/spark-dashboard/commit/e499da9c94047fa0d848b8649fe9ee0bc9aafe63))
* **ui:** DataPoint field is timestamp not t (tsc); copied domain ([5776f7e](https://github.com/Wpnx330/spark-dashboard/commit/5776f7efe95d72d1adbb58f1befcedc5f1c3234a))
* **ui:** disable padData on live charts — differing per-series fake-heads double-merged timestamps, stretching 5m x-domain to 10m ([a8b1934](https://github.com/Wpnx330/spark-dashboard/commit/a8b1934b64a512b5f610cd1abd7cdc01fbf205b3))
* **ui:** flush metrics at 1s — 2s pending-overwrite halved buffer density, squeezing 1m view to 2min/ 5m to half density ([383249a](https://github.com/Wpnx330/spark-dashboard/commit/383249a9f00e5528739b0e3e3f43c0c61a45cecd))
* **ui:** time-pin live 1m/5m domains + 3-tick axis (start/mid/now) ([dde03c2](https://github.com/Wpnx330/spark-dashboard/commit/dde03c27f1cb21bf1fea85df4c899c8343b10175))
* useNodes hook expected wrapped response, API returns bare array ([47ced27](https://github.com/Wpnx330/spark-dashboard/commit/47ced2796ce9da4724dade30a2c0cf878fb15b67))
* **vllm:** quiet expected HuggingFace enrichment misses ([e1c079d](https://github.com/Wpnx330/spark-dashboard/commit/e1c079db220fd6b37dedc4af50612c3046067d27))


### Reverts

* restore upstream flex layout — engine shrink-0, hardware flex-1 ([24b9d81](https://github.com/Wpnx330/spark-dashboard/commit/24b9d81ae25d56918cd4d4f9eeadda1f5cdce589))


### Dependencies & Chores

* **ci:** bump github actions to latest stable ([49b68bd](https://github.com/Wpnx330/spark-dashboard/commit/49b68bd3d21d8b4ce77a50b4aaaa1b483993592a))
* **ci:** remove crates.io publish machinery ([2428ee4](https://github.com/Wpnx330/spark-dashboard/commit/2428ee43ca97d17607b0379a813b8f17be906356))
* **deploy:** 0827 daily backfill SQL (executed on DGX1) ([b579fa2](https://github.com/Wpnx330/spark-dashboard/commit/b579fa266ce4a784b51d103a0c75e39e977ba111))
* **deps:** align @types/node with the Node 24 LTS toolchain ([9dca215](https://github.com/Wpnx330/spark-dashboard/commit/9dca21573fad9674f0e0ea349fe42564a66b3579))
* **deps:** bump docker base images to latest stable ([357cd0f](https://github.com/Wpnx330/spark-dashboard/commit/357cd0fb90604c930f59194f1fb76f3040784792))
* **deps:** bump Docker builder image to rust:1.97-slim ([bb97a34](https://github.com/Wpnx330/spark-dashboard/commit/bb97a3494c410ea7864925b9f8f41f514ba91c3c))
* **deps:** bump frontend deps to latest stable ([124ce79](https://github.com/Wpnx330/spark-dashboard/commit/124ce7983b67da628517592ebcc4bed02e03ce38))
* **deps:** bump rust crates to latest stable ([32d4e42](https://github.com/Wpnx330/spark-dashboard/commit/32d4e420327cbea34d979c4000d59d61f068f15e))
* **deps:** refresh npm lockfile to latest in-range versions ([2e1bf67](https://github.com/Wpnx330/spark-dashboard/commit/2e1bf67646e690ccfba814a9f5ede627e1ddfc51))
* **deps:** update all dependencies ([#22](https://github.com/Wpnx330/spark-dashboard/issues/22)) ([e6d71e4](https://github.com/Wpnx330/spark-dashboard/commit/e6d71e4f1f4b54bb5aa6b6aa8a79bde203170ced))
* **deps:** update all dependencies to latest stable ([2e596b4](https://github.com/Wpnx330/spark-dashboard/commit/2e596b46cc0a0a1a7cb59dca5f0a8e135daaef41))
* **dev:** forward SPARK_DASHBOARD_SIMULATE_GPUS in the bare-metal dev loop ([cd3290c](https://github.com/Wpnx330/spark-dashboard/commit/cd3290cbc0159c8917bb5eccf39b7da27b41233f))
* **main:** release spark-dashboard 0.10.0 ([fd7f966](https://github.com/Wpnx330/spark-dashboard/commit/fd7f966b883adb4179fa07a9d04155ca05c43a5a))
* **main:** release spark-dashboard 0.11.0 ([4036dec](https://github.com/Wpnx330/spark-dashboard/commit/4036dec43557c30e1f168e1196670339325192d5))
* **main:** release spark-dashboard 0.12.0 ([3ed5035](https://github.com/Wpnx330/spark-dashboard/commit/3ed503578f459db58d35c6061b51d99b1c839167))
* **main:** release spark-dashboard 0.2.0 ([7cd3939](https://github.com/Wpnx330/spark-dashboard/commit/7cd3939131526094cc3a00b0b3db45b4d1106b95))
* **main:** release spark-dashboard 0.2.0 ([30af5fa](https://github.com/Wpnx330/spark-dashboard/commit/30af5fa93acef78ebdc0bfd1658a09a204727aa1))
* **main:** release spark-dashboard 0.3.0 ([dd50b3c](https://github.com/Wpnx330/spark-dashboard/commit/dd50b3c50772a3dc111a2ec7d967e65bf5e240dd))
* **main:** release spark-dashboard 0.3.0 ([9ee13c6](https://github.com/Wpnx330/spark-dashboard/commit/9ee13c67570c2fd00f8a7e1cbc6efb7efcb61692))
* **main:** release spark-dashboard 0.4.0 ([#13](https://github.com/Wpnx330/spark-dashboard/issues/13)) ([ff47f78](https://github.com/Wpnx330/spark-dashboard/commit/ff47f78f6ca8ec4fc6f8e9c32605dee9cd10c477))
* **main:** release spark-dashboard 0.5.0 ([#15](https://github.com/Wpnx330/spark-dashboard/issues/15)) ([f65e3bb](https://github.com/Wpnx330/spark-dashboard/commit/f65e3bb234a54d53c3f46b433d5fa2d4094e443f))
* **main:** release spark-dashboard 0.6.0 ([#19](https://github.com/Wpnx330/spark-dashboard/issues/19)) ([9ee772c](https://github.com/Wpnx330/spark-dashboard/commit/9ee772c7748dc3a6a4847b0a4d2baa0288ff25bb))
* **main:** release spark-dashboard 0.7.0 ([#21](https://github.com/Wpnx330/spark-dashboard/issues/21)) ([5c9be67](https://github.com/Wpnx330/spark-dashboard/commit/5c9be67332a90bccd6c257814fadefed4e838b5a))
* **main:** release spark-dashboard 0.8.0 ([#24](https://github.com/Wpnx330/spark-dashboard/issues/24)) ([4093095](https://github.com/Wpnx330/spark-dashboard/commit/4093095e880879c9cc82374d7f040e7175a8604e))
* **main:** release spark-dashboard 0.9.0 ([802a663](https://github.com/Wpnx330/spark-dashboard/commit/802a663a80d351466a4ac09f316713637dd90e49))
* strip session artifacts from deploy snapshot ([15df7c2](https://github.com/Wpnx330/spark-dashboard/commit/15df7c23919b2c520b7d0fd744a68cccc7edab13))
* surface dependency & chore commits in release notes ([d650799](https://github.com/Wpnx330/spark-dashboard/commit/d650799c3b106f7c5937deffd14dd8b59bbd7139))
* **sync:** base commit = deployed DGX1 tree (binary source of truth, Oct 3 build) ([4bc8600](https://github.com/Wpnx330/spark-dashboard/commit/4bc86006ba729fa999cd5d4de2fc9f404ccf0834))

## [0.12.0](https://github.com/niklasfrick/spark-dashboard/compare/spark-dashboard-v0.11.0...spark-dashboard-v0.12.0) (2026-07-21)


### Features

* **engines:** associate detected engines with the GPUs they run on ([e406a50](https://github.com/niklasfrick/spark-dashboard/commit/e406a505f52a606a1b6a0138d8e49cc81e660fc0))
* **frontend:** show per-engine GPU badge on multi-GPU hosts ([180e414](https://github.com/niklasfrick/spark-dashboard/commit/180e414f39550b5b1ace70918e0f749add6a7dd6)), closes [#45](https://github.com/niklasfrick/spark-dashboard/issues/45)
* **gpu:** add --simulate-gpus flag appending fictive GPUs to snapshots ([f6e29c5](https://github.com/niklasfrick/spark-dashboard/commit/f6e29c5b49ebb9178f3b03031aba4e95a0b02b32))
* **gpu:** make main Dashboard hardware panels switch between GPUs ([a34c31a](https://github.com/niklasfrick/spark-dashboard/commit/a34c31ab718bdd88d1cc4d955e71cecc00024670)), closes [#44](https://github.com/niklasfrick/spark-dashboard/issues/44)
* **gpu:** monitor all NVIDIA GPUs while preserving backward compatibility ([8d6033c](https://github.com/niklasfrick/spark-dashboard/commit/8d6033c3f94b6c00e50922aa1dbd423642441942))


### Bug Fixes

* **engines:** clear stale engine PIDs and test the detection merge ([6970153](https://github.com/niklasfrick/spark-dashboard/commit/69701531a5217ba2ba2fe1c7cd4d2d2c5baf0e37))
* **gpu:** keep Dashboard hook order stable across the first snapshot ([2afad09](https://github.com/niklasfrick/spark-dashboard/commit/2afad09e8b3be91c7c4a31bc942f8e59f0c45c11))


### Dependencies & Chores

* **dev:** forward SPARK_DASHBOARD_SIMULATE_GPUS in the bare-metal dev loop ([cd3290c](https://github.com/niklasfrick/spark-dashboard/commit/cd3290cbc0159c8917bb5eccf39b7da27b41233f))

## [0.11.0](https://github.com/niklasfrick/spark-dashboard/compare/spark-dashboard-v0.10.0...spark-dashboard-v0.11.0) (2026-06-18)


### Features

* **docker:** harden runtime image with distroless base and self-probe healthcheck ([1ccc1db](https://github.com/niklasfrick/spark-dashboard/commit/1ccc1dbce75147a2667ecfc40549250d5d122fff))
* **engines:** parse vLLM speculative-decoding metrics ([ac512e9](https://github.com/niklasfrick/spark-dashboard/commit/ac512e9bca984b846834c43dc59949f07b513ae7))
* **frontend:** adapt hardware cards to vertical space ([68ee1ff](https://github.com/niklasfrick/spark-dashboard/commit/68ee1fffa58cafb9ad1dcb5c0d58b9e46dee1451))
* **frontend:** surface speculative-decoding metrics in cache card ([feb936a](https://github.com/niklasfrick/spark-dashboard/commit/feb936a1c78a71c6024b8570e0dcef2171194293))


### Bug Fixes

* **frontend:** scale GPU power gauge by observed peak when no cap ([e2d525e](https://github.com/niklasfrick/spark-dashboard/commit/e2d525eddc7a6becd55bccab5d9179986c6b4bcc))
* **metrics:** resolve GPU power limit via NVML fallback chain ([e68d7cb](https://github.com/niklasfrick/spark-dashboard/commit/e68d7cb2725d1f69f4a6648fe8738943ad24e776))


### Dependencies & Chores

* **ci:** bump github actions to latest stable ([49b68bd](https://github.com/niklasfrick/spark-dashboard/commit/49b68bd3d21d8b4ce77a50b4aaaa1b483993592a))
* **deps:** bump frontend deps to latest stable ([124ce79](https://github.com/niklasfrick/spark-dashboard/commit/124ce7983b67da628517592ebcc4bed02e03ce38))
* **deps:** bump rust crates to latest stable ([32d4e42](https://github.com/niklasfrick/spark-dashboard/commit/32d4e420327cbea34d979c4000d59d61f068f15e))

## [0.10.0](https://github.com/niklasfrick/spark-dashboard/compare/spark-dashboard-v0.9.0...spark-dashboard-v0.10.0) (2026-06-17)


### Features

* **dev:** add docker-dev.sh container test harness ([b6bbdca](https://github.com/niklasfrick/spark-dashboard/commit/b6bbdca008028de66bde77ecc6faed43c9cd0644))
* **docker:** add hardened multi-stage image and compose deployment ([6d0d700](https://github.com/niklasfrick/spark-dashboard/commit/6d0d700f72c4a5530062a2f0bc25873b6b814d7c))
* **server:** add /healthz liveness endpoint ([d7bc7f7](https://github.com/niklasfrick/spark-dashboard/commit/d7bc7f7f95c8cf1f72e0c9ed12510707597bc187))


### Bug Fixes

* **vllm:** quiet expected HuggingFace enrichment misses ([e1c079d](https://github.com/niklasfrick/spark-dashboard/commit/e1c079db220fd6b37dedc4af50612c3046067d27))


### Dependencies & Chores

* **deps:** bump docker base images to latest stable ([357cd0f](https://github.com/niklasfrick/spark-dashboard/commit/357cd0fb90604c930f59194f1fb76f3040784792))
* surface dependency & chore commits in release notes ([d650799](https://github.com/niklasfrick/spark-dashboard/commit/d650799c3b106f7c5937deffd14dd8b59bbd7139))

## [0.9.0](https://github.com/niklasfrick/spark-dashboard/compare/spark-dashboard-v0.8.0...spark-dashboard-v0.9.0) (2026-05-19)


### Features

* add cumulative token totals to engine throughput cards ([dd80c8e](https://github.com/niklasfrick/spark-dashboard/commit/dd80c8efe7664bb4755631393c447ae269064e2a))
* add Time Per Output Token (TPOT) to engine latency card ([20bc677](https://github.com/niklasfrick/spark-dashboard/commit/20bc6772ce70320b330dd28656c4f87a11cb21e5))
* bracket chart units in titles and clean up hover tooltips ([d92a975](https://github.com/niklasfrick/spark-dashboard/commit/d92a975f8537a3aed448410c4c1c0a7ed8ea89f6))
* cache resolved model info and support per-endpoint engine API keys ([18fa2f8](https://github.com/niklasfrick/spark-dashboard/commit/18fa2f8ff53c6d9d60577e9a375e28b23d78397b))
* hide tooltip header on prefill and decode throughput charts too ([952d5ee](https://github.com/niklasfrick/spark-dashboard/commit/952d5ee85f680045217aefaf49bf9bd8baa57e65))
* move tok/s into engine throughput titles and fix hover header ([bc2292f](https://github.com/niklasfrick/spark-dashboard/commit/bc2292f4eb6315534dbe48078fd6342ed0dd9c33))
* plot prefix cache hit rate alongside KV cache over time ([4a105ca](https://github.com/niklasfrick/spark-dashboard/commit/4a105cae7dc1aaabdd2a090f5cf8bc53f51476af))
* select physical/Wi-Fi network interface instead of loopback ([d7cdfbf](https://github.com/niklasfrick/spark-dashboard/commit/d7cdfbfc38b6cd10d49f976e1ecd4cc711abd0bb))
* surface cumulative prefix cache queries on engine cache card ([0860324](https://github.com/niklasfrick/spark-dashboard/commit/08603248ddb94308d8ea4a24a83ace4cb6ab5051))

## [0.8.0](https://github.com/niklasfrick/spark-dashboard/compare/spark-dashboard-v0.7.0...spark-dashboard-v0.8.0) (2026-05-01)


### Features

* slo goodput customization ([#23](https://github.com/niklasfrick/spark-dashboard/issues/23)) ([1576e43](https://github.com/niklasfrick/spark-dashboard/commit/1576e43f83460d3dfefc7145086a22b289dd8ac9))

## [0.7.0](https://github.com/niklasfrick/spark-dashboard/compare/spark-dashboard-v0.6.0...spark-dashboard-v0.7.0) (2026-04-28)


### Features

* improvind dashboard and fixing data visibility ([#20](https://github.com/niklasfrick/spark-dashboard/issues/20)) ([c7851bd](https://github.com/niklasfrick/spark-dashboard/commit/c7851bd496f4d8c12b2cda163705e583b44fce6a))

## [0.6.0](https://github.com/niklasfrick/spark-dashboard/compare/spark-dashboard-v0.5.0...spark-dashboard-v0.6.0) (2026-04-28)


### Features

* model detail tags ([#18](https://github.com/niklasfrick/spark-dashboard/issues/18)) ([c0633bc](https://github.com/niklasfrick/spark-dashboard/commit/c0633bc9fe8d98499713084899b781773d38d99b))

## [0.5.0](https://github.com/niklasfrick/spark-dashboard/compare/spark-dashboard-v0.4.0...spark-dashboard-v0.5.0) (2026-04-25)


### Features

* **vllm:** expand vLLM observability with latency percentiles, SLO goodput, and dashboard polish ([082bd17](https://github.com/niklasfrick/spark-dashboard/commit/082bd17adcaf4da6dc577ce1cefcd91b90464cae))


### Bug Fixes

* compute vLLM prefix cache hit rate from counters ([#14](https://github.com/niklasfrick/spark-dashboard/issues/14)) ([1797e8a](https://github.com/niklasfrick/spark-dashboard/commit/1797e8a1b8f8208176771c727cd3d2a98dac4e10))

## [0.4.0](https://github.com/niklasfrick/spark-dashboard/compare/spark-dashboard-v0.3.0...spark-dashboard-v0.4.0) (2026-04-23)


### Features

* multi-engine dashboard with auto-rotation and live vLLM metrics ([#12](https://github.com/niklasfrick/spark-dashboard/issues/12)) ([53b2af8](https://github.com/niklasfrick/spark-dashboard/commit/53b2af81b0a21f2779bfac26e5a6bc2546d8490e))

## [0.3.0](https://github.com/niklasfrick/spark-dashboard/compare/spark-dashboard-v0.2.0...spark-dashboard-v0.3.0) (2026-04-22)


### Features

* make dashboard hardware- and host-agnostic ([3b77d5a](https://github.com/niklasfrick/spark-dashboard/commit/3b77d5a4ee95bdfd316f56a256a34ee06e339bda))
* make dashboard hardware- and host-agnostic ([9f0e3a6](https://github.com/niklasfrick/spark-dashboard/commit/9f0e3a6d2b321f42736a1ac03551da9b87072da2))
* surface engine deployment mode (Docker vs Direct) in tabs ([6e9de44](https://github.com/niklasfrick/spark-dashboard/commit/6e9de441bc13b0d451bbb47808373e3d8926b6d0))

## [0.2.0](https://github.com/niklasfrick/spark-dashboard/compare/spark-dashboard-v0.1.0...spark-dashboard-v0.2.0) (2026-04-20)


### Features

* package and distribute via cargo install + systemd service ([b2a87d8](https://github.com/niklasfrick/spark-dashboard/commit/b2a87d8d42f29bd105c456bed581e657d6edafa0))


### Bug Fixes

* **dev:** prevent local tilde expansion of SPARK_DIR ([e1e8350](https://github.com/niklasfrick/spark-dashboard/commit/e1e8350d499c5b6c12470dee536d5d1c3be0b10b))
* **install:** refuse sudo invocation; let binary self-escalate ([942c796](https://github.com/niklasfrick/spark-dashboard/commit/942c7961b48fc01771ec5a34af659c4e8a2ddd5e))
* **install:** refuse sudo invocation; let binary self-escalate ([e778623](https://github.com/niklasfrick/spark-dashboard/commit/e77862341ddd62581e643643977c6cc9aa473bbe))
* **test:** align MemoryCard test selectors with current StackedBar markup ([e499da9](https://github.com/niklasfrick/spark-dashboard/commit/e499da9c94047fa0d848b8649fe9ee0bc9aafe63))
