# Слайс N — чеклист волн (имена тестов ≤ 5 сегментов)

План и контекст: [sprint-15-slice-N-plan.md](sprint-15-slice-N-plan.md).

Отмечать волну `[x]` только после **`pwm-review` PASS или PARTIAL без блокеров** и записи **`commits[]`** в тикете волны.

- [x] **N.1** — `pwmd` деревья **`src/tests/**`**, **`src/transport/tests/**`**: короткие **`#[test] fn`** / хелперы + **`///`**; тикет `tasks/20260615-s15-slice-N-wave1-pwmd-test-fn-names.json`; **`28a3ec2`** / **`3d25ddb`** / документирование (**git log**); ревью **`docs/reviews/sprint-15-slice-N-wave1-pwmd-test-fn-names-review.md`** (**PASS**).
- [x] **N.2** — **`pwm-cli`** **`src/tests/mod.rs`** + **`tests/*.rs`**: тикет `tasks/20260616-s15-slice-N-wave2-pwm-cli-test-fn-names.json`; **`2de256c`** / **`5f11691`** / документирование (**git log**); ревью **`docs/reviews/sprint-15-slice-N-wave2-pwm-cli-test-fn-names-review.md`** (**PASS**).
- [x] **N.3** — **`pwm-tui`** **`tests/**/*.rs`**: тикет `tasks/20260617-s15-slice-N-wave3-pwm-tui-tests-fn-names.json`; **`cff2fab`** / **`3343f11`** / **`64e8c49`** / документирование (**git log**); ревью **`docs/reviews/sprint-15-slice-N-wave3-pwm-tui-tests-fn-names-review.md`** (**PASS**).
- [x] **N.4** — inline **`#[cfg(test)]`**: **`pwm-cli`** (**`wallet/mod`**, **`bruteforce`**), **`pwm-core`**, **`pwmd`**; тикет `tasks/20260618-s15-slice-N-wave4-inline-tests-fn-names.json`; **`4a1523c`** / документирование (**git log**); ревью **`docs/reviews/sprint-15-slice-N-wave4-final-review.md`** (**PASS**).
