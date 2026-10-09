# Signals and math

The Rust signal interface exports timestamped research candidates as JSONL. It does not connect accounts or submit orders. A candidate includes declared model/evidence identity, a conservative forecast, exact fee/edge arithmetic, budget, share count and an optional Engine trade event. The suggested event still passes through current book, cash, city, inventory, time, rate and halt checks when consumed by Engine.

```sh
make signals evaluate
.local/bin/hft-signals fixtures/signals.jsonl
.local/bin/hft-signals - < fixtures/signals.jsonl
.local/bin/hft-evaluate fixtures/paired-forecasts.json
```

## Exact binary-contract math

All values below are in integer millionths. A share resolves to 1,000,000 on success and zero otherwise. For declared probability p, ask a and paper fee f:

- Effective cost q = a + ceil(a × fee_bps / 10,000).
- Expected edge per share = p − q.
- Positive-edge full Kelly fraction = floor((p − q) × 1,000,000 / (1,000,000 − q)).
- Optional kelly_multiplier_micro applies fractional Kelly before the cap, with downward rounding; omitted values retain full Kelly for compatibility.
- Cash fraction is capped by max_fraction_micro, then floored using u128 intermediates.
- Order budget is capped by max_order_micro. Whole shares are floored and capped by max_shares.

Nonpositive edge or cost at/above the resolution payout produces zero Kelly allocation. The signal subtracts uncertainty_margin_micro from p before computing these quantities. This is a user-declared safety margin, not a statistical confidence interval. Dollar sizing assumes the supplied probability is appropriate and the flat upfront fee model applies. Portfolio dependence, live fee schedules, gas and queue effects are absent.

Kelly is a research sizing rule motivated by [Kelly's original paper](https://onlinelibrary.wiley.com/doi/10.1002/j.1538-7305.1956.tb03809.x). Neither a positive expected edge nor a proposed share count establishes a realized profit.

Signal requests reject unsupported chains, future/stale clocks, missing identity/evidence and invalid probability/budget inputs. Expiry uses the older quote/forecast timestamp. The stateless CLI does not deduplicate signal IDs or reserve inventory/cash. Downstream consumers must apply idempotence, expiration and execution risk.

## Weather conditioning correction

For an observed high o, the normal baseline is conditioned on X ≥ o. For an observed low it is conditioned on X ≤ o. Each integer bin receives a half-unit continuity correction, is clipped to the permitted support, and is divided by the conditional tail mass. With zero hours remaining, the observed bin receives probability one.

Earlier code moved excluded mass into the boundary bin, which is censoring rather than truncation. The current implementation uses conditional normalization. A half-normal golden test checks bin [-0.5, 0.5] at observed zero, mean zero and sigma one: probability approximately 0.382924922548. Grid tests check support, symmetry and total mass. The CDF remains a floating-point approximation; these tests do not constitute a formal proof of all inputs.

The normalization follows the [SciPy truncated-normal formula](https://scipy.github.io/devdocs/tutorial/stats/continuous_truncnorm.html). Missing partition mass and nonfinite/invalid probabilities fail. The weather baseline remains untrained; forecast changes affect weather-triggered candidates and require new held-out calibration.

## Paired model evaluation

hft-evaluate scores two supplied forecast columns against exactly the same resolved outcomes. Input must identify candidate/reference models, dataset and test/heldout split. Duplicate IDs, invalid probabilities and declared forecasts at/after resolution reject. Declared timestamps and dataset provenance are not independently authenticated.

The report includes Brier score, clipped log loss, calibration error and paired differences. A descriptive standard error is available for multiple records; time-series dependence can invalidate independent-sample interpretations. No inference latency, model loading, training or statistical significance is established.

fixtures/paired-forecasts.json is an explicitly synthetic scoring demonstration against constant 0.5. It is not an FPT run. An FPT superiority claim requires the exact model, compatible task, common held-out data and comparable inference setup.

## Deployment contract

Package hft-signals as an input/output worker and feed bounded JSONL through stdin. Store emitted records with source/evaluation artifacts so consumers can inspect each candidate. There is no public server or authenticated production deployment in this release. The engine remains paper-only and Base-only.

For fundraising or commercial evaluation, present measured operation benchmarks, reproducible code, forecast evidence and remaining gates. This project currently has no verified live revenue, fund returns, assets under management or investor commitments.
