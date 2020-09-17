.PHONY: all format format-check clean

all:
	@$(MAKE) --no-print-directory -C libos
	@$(MAKE) --no-print-directory -C pal
	@$(MAKE) --no-print-directory -C run
	@$(MAKE) --no-print-directory -C exec

format:
	@$(MAKE) --no-print-directory -C libos format
	@$(MAKE) --no-print-directory -C pal format
	@$(MAKE) --no-print-directory -C run format
	@$(MAKE) --no-print-directory -C exec format

format-check:
	@$(MAKE) --no-print-directory -C libos format-check
	@$(MAKE) --no-print-directory -C pal format-check
	@$(MAKE) --no-print-directory -C run format-check
	@$(MAKE) --no-print-directory -C exec format-check

clean:
	@$(MAKE) --no-print-directory -C libos clean
	@$(MAKE) --no-print-directory -C pal clean
	@$(MAKE) --no-print-directory -C run clean
	@$(MAKE) --no-print-directory -C exec clean
