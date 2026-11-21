# Changelog

## [0.1.2](https://github.com/jalet/spotprice-exporter/compare/v0.1.1...v0.1.2) (2026-11-21)


### Bug Fixes

* **deps:** adapt A44 parser to quick-xml 0.42 ([3478611](https://github.com/jalet/spotprice-exporter/commit/34786112b60d0ff224db7822784153feb6cad463))
* **deps:** update rust crate quick-xml to 0.42.0 ([2f830c4](https://github.com/jalet/spotprice-exporter/commit/2f830c46fc098de09d8432eb7351b36f3530aed0))

## [0.1.1](https://github.com/jalet/spotprice-exporter/compare/v0.1.0...v0.1.1) (2026-11-21)


### Bug Fixes

* **chart:** make the Cilium collector port configurable ([1d0644c](https://github.com/jalet/spotprice-exporter/commit/1d0644c8fa6a3be82bea429a3a78502660555684))
* **provider:** log transport kind and response excerpt ([cf2cf1c](https://github.com/jalet/spotprice-exporter/commit/cf2cf1c75c607bd64175286456863ed22319bee8))
* **provider:** log transport kind and response excerpt ([9490ae9](https://github.com/jalet/spotprice-exporter/commit/9490ae9bd66ed55c30b65cdcf74ad10e4f1cd3c8))

## 0.1.0 (2026-11-18)


### Features

* **config:** resolve bidding zone names through a registry ([7a5cce1](https://github.com/jalet/spotprice-exporter/commit/7a5cce153cde77897e47a3f3345016fc686797d7))
* **derived:** add local-day statistics and cheapest window ([05e6ef2](https://github.com/jalet/spotprice-exporter/commit/05e6ef2a736708d78569de5450f6006124eeeb03))
* **metrics:** export local-day statistics and cheapest window ([d2fd8bf](https://github.com/jalet/spotprice-exporter/commit/d2fd8bf266106292460d7e943cec7bb295d5beef))
* scaffold spotprice exporter with OTLP metrics ([e27dbac](https://github.com/jalet/spotprice-exporter/commit/e27dbac8f4dd0c115fc886c4aa22fcc653c205b7))
* **scheduler:** refresh each zone hourly to fill source gaps ([a641780](https://github.com/jalet/spotprice-exporter/commit/a641780529c8ac016a19be4c5d961ddf0545e2bc))


### Bug Fixes

* **provider:** keep TimeSeries blocks separate in A44 documents ([0049a0b](https://github.com/jalet/spotprice-exporter/commit/0049a0b3e88c562f3e181a9dc267ef92732d4df1))
* **provider:** use the SDAC sequence for DE-LU day-ahead prices ([f1e5307](https://github.com/jalet/spotprice-exporter/commit/f1e53071f62445bace7328c77d4ae198dc10573e))
