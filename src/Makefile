.PHONY: all clean

all:
	@$(MAKE) --no-print-directory -C libos
	@$(MAKE) --no-print-directory -C pal
	@$(MAKE) --no-print-directory -C run

clean:
	@$(MAKE) --no-print-directory -C libos clean
	@$(MAKE) --no-print-directory -C pal clean
	@$(MAKE) --no-print-directory -C run clean
