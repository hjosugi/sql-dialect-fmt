# Deterministic E2E expectations

After `00_bootstrap.sql` through `07_assertions.sql` run successfully:

| Assertion | Expected |
|---|---:|
| `CORE.CUSTOMERS` rows | 6 |
| `CORE.ORDERS` rows | 5 |
| `CORE.ORDER_ITEMS` rows | 8 |
| `CORE.MULTILINGUAL_TEXTS` rows | 9 |
| `MART.V_CUSTOMER_360` rows | 6 |
| customer `C006` | absent |
| customer `C007` with locale `th-TH` | present |
| order `O1004` | absent |
| order `O1002` status | `SHIPPED` |
| `OPS.ERROR_LOG` rows | 0 |

The task graph exists but remains suspended. The optional `COPY INTO` file is not part of
these deterministic counts.
