include ../test_common.mk

ARGC := 4
ARG1 := 1
ARG2 := arg2
ARG3 := this is a string with spaces

EXTRA_C_FLAGS := \
	-DEXPECTED_ARGC=$(ARGC) \
	-DEXPECTED_ARG1="\"$(ARG1)\"" \
	-DEXPECTED_ARG2="\"$(ARG2)\"" \
	-DEXPECTED_ARG3="\"$(ARG3)\""
EXTRA_LINK_FLAGS :=
BIN_ARGS := "$(ARG1)" "$(ARG2)" "$(ARG3)"
