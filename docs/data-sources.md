# Data sources

Status of each source, and what is verified. Dates are when the check was made.

## ENTSO-E Transparency Platform (v1 provider)

Document type A44 (day-ahead prices). Endpoint `https://web-api.tp.entsoe.eu/api`.

### Access and token (checked 2026-11-17)

- Register on the Transparency Platform and verify the account by email.
- Send an email to transparency@entsoe.eu, subject "RESTful API access", with the registered address in the body.
- Approval takes up to 3 working days. Then generate the token under "My Account".
- Source: ENTSO-E helpdesk, <https://transparencyplatform.zendesk.com/hc/en-us/articles/12845911031188>
  (read via search snippet, not fetched directly).
- Token is passed as the `securityToken` query parameter. A missing or invalid token returns HTTP 401.

### Rate limit (partly verified)

- Primary limit: 400 requests per minute per API token, per the ENTSO-E helpdesk article
  <https://transparencyplatform.zendesk.com/hc/en-us/articles/12783148966036-API-Rate-Limit-Part-1>.
  The article itself returned HTTP 403 to the fetcher, so the figure is from a search snippet and
  a third-party catalogue. **Confirm with a manual test** before relying on it.
- Exceeding the limit returns HTTP 429 (per a Fingrid mirror of the API, not ENTSO-E directly).
- No daily quota was found. The poller uses a handful of requests per day, far below the limit.

### Terms of use and licence (NOT RESOLVED)

**VERIFY WITH LEGAL COUNSEL before publishing a public image or redistributing data.**

The sources found disagree:

- A 2019 ENTSO-E announcement says TSOs agreed to release data under an open licence, so users need
  no permission from data owners before reuse. Source: <https://www.entsoe.eu/news/2019/02/01/tsos-increase-number-of-open-data-available-through-entso-e-s-transparency-platform/>
- An openmod discussion says the Terms and Conditions do not give a proper licence, and users must
  check with each primary data owner. Source: <https://groups.google.com/g/openmod-initiative/c/C9NsEpyAzJY>
- A 2016 ETUG minutes record refers to a copyright provision added to the Terms of Use in January 2015.
- The ENTSO-E website disclaimer prohibits use of content beyond the Terms of Use without written permission.

The current Terms of Use page (transparency.entsoe.eu, knowledge base) returned HTTP 400 to the fetcher.
Read it directly and record: the licence, attribution wording, and whether redistribution or
commercial use is allowed.

Until this is resolved, the exporter is for private use. Do not publish a public image that
redistributes ENTSO-E data.

## Other sources (not used in v1)

| Source | Coverage | Auth | Status |
| --- | --- | --- | --- |
| elprisetjustnu.se | SE1-SE4 | none | Candidate for S4.1. Terms unclear, source credit requested on public display. Resolution 15 min from 2025-10-01 (research pass, not verified). |
| energi-data-service (Energinet) | DK | none reported | Candidate for S4.1. `Elspotprices` replaced by `DayAheadPrices` (research pass, not verified). |
| Nord Pool Data Portal | multiple | paid API | Not used. Official API is paid. |
| aWATTar | DE, AT | none | Not used in v1. Hourly only (not verified). |

## Publication timing (to measure in S0.2)

- Day-ahead market clears around 12:45 CET. Next-day prices become available from about 13:00 (other
  sources; ENTSO-E latency not verified).
- Polling after the publication window with backoff. Measured values go here once S0.2 runs.
