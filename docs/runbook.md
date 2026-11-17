# Runbook

Symptoms in the order you are likely to see them. Commands assume a Kubernetes deployment; adjust
the namespace and pod names.

## No price data at all (`/ready` returns 503)

The exporter has not fetched any prices since start.

1. Check the logs for the fetch outcome:
   `kubectl logs deploy/spotprice-exporter | grep -E "price fetch|prices fetched"`
2. If `unauthorized` appears: the token is wrong or expired. Check the secret, then restart the pod.
   Fetches retry hourly after an authentication failure, so a restart is faster than waiting.
3. If `rate_limited` appears: the token exceeds ENTSO-E's per-token limit (400 requests per minute,
   per the helpdesk snippet, to be confirmed). Check for other clients using the same token.
4. If `transient` appears: the ENTSO-E API or the network is failing. Backoff grows up to 60 minutes.
   Check `curl -sv "https://web-api.tp.entsoe.eu/api"` from the pod's network.
5. If `no_data` appears only: check the zone code. A wrong EIC code returns no data, not an error.

## Stale data (`energy.market.data.age` is growing)

The last successful fetch is old. The gauge still reports the last good prices, so consumers see
stale values rather than gaps.

1. Look at `energy.market.fetch.attempts` by `energy.market.fetch.result`. Find the dominant failure.
2. Next-day prices are only expected after about 12:45 CET (11:00 UTC is the scheduler's early
   attempt). If `no_data` is the only result before then, that is normal.
3. If the data age is over two hours after 13:00 CET, treat it as a real problem and follow the
   steps for the dominant result above.

## Coverage running out (`energy.market.data.horizon` near zero or negative)

Prices are cached only up to the horizon. Once it reaches zero, `energy.market.price` is absent for
future intervals.

1. Check that the next day's fetch succeeded. Coverage should extend past midnight UTC by early
   afternoon.
2. If the horizon is negative and fetches succeed, the zone may have no published prices for the
   window. Check the zone code and the ENTSO-E web UI for that zone.

## Metrics missing from the collector

1. Check `OTEL_EXPORTER_OTLP_ENDPOINT` in the pod environment, and that the collector accepts OTLP/HTTP
   on the port in use (4318 by default).
2. Check the NetworkPolicy or CiliumNetworkPolicy. The collector must be allowed in the egress rules.
3. Export runs every 60 seconds by default. Allow one interval after startup before concluding that
   nothing is exported.
4. Shutdown flushes buffered metrics. If a pod is killed without SIGTERM (for example OOM), the last
   interval can be lost.

## Clean shutdown

SIGTERM stops the health server, aborts the scheduler and flushes metrics. Look for
`spotprice-exporter stopped` in the logs. A missing line means the process was killed.
