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

### Terms of use and licence (confirmed by the owner, 2026-11-18)

The owner has confirmed that the licence is acceptable for this use. The terms were not re-read
by the exporter's authors. If the image is ever published publicly, re-read the current Terms of
Use and keep the attribution it asks for.

Background, for reference: the 2019 ENTSO-E announcement on open data is the most permissive
source found. Other sources disagree, and the terms page itself could not be fetched.

## Other sources (not used in v1)

| Source | Coverage | Auth | Status |
| --- | --- | --- | --- |
| elprisetjustnu.se | SE1-SE4 | none | Candidate for S4.1. Terms unclear, source credit requested on public display. Resolution 15 min from 2025-10-01 (research pass, not verified). |
| energi-data-service (Energinet) | DK | none reported | Candidate for S4.1. `Elspotprices` replaced by `DayAheadPrices` (research pass, not verified). |
| Nord Pool Data Portal | multiple | paid API | Not used. Official API is paid. |
| aWATTar | DE, AT | none | Not used in v1. Hourly only (not verified). |

## Publication timing (to measure in S0.2)

Day-ahead results clear once a day, on a fixed schedule (around 12:45 CET). What is not yet known is
when the data appears on the API. The S0.2 poll measures that.
