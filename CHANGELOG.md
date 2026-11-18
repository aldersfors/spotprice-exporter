# Changelog

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
