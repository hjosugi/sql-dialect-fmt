.PHONY: check list golden

check:
	python tools/lexical_checks.py
	python tools/embedded_language_checks.py
	python tools/scenario_checks.py

list:
	find cases e2e -type f | sort

golden:
	@test -n "$(FORMATTER)" || (echo 'Set FORMATTER, e.g. make golden FORMATTER="tool --write {file}"' && exit 2)
	python tools/assert_formatter.py --formatter '$(FORMATTER)' --profile full
