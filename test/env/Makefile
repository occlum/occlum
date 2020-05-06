include ../test_common.mk

ARGC := 4
ARG1 := 1
ARG2 := arg2
ARG3 := this is a string with spaces

EXTRA_C_FLAGS := \
	-DEXPECT_ARGC=$(ARGC) \
	-DEXPECT_ARG1="\"$(ARG1)\"" \
	-DEXPECT_ARG2="\"$(ARG2)\"" \
	-DEXPECT_ARG3="\"$(ARG3)\""
EXTRA_LINK_FLAGS :=
EXTRA_ENV := TEST=true STABLE=no OVERRIDE=Y
BIN_ARGS := "$(ARG1)" "$(ARG2)" "$(ARG3)"
