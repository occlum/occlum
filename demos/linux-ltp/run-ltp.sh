#!/bin/sh

setup()
{
    export LTPROOT=${LTPROOT:-/opt/ltp}
    export TMP="/tmp"
    export PATH="${PATH}:${LTPROOT}/testcases/bin:${LTPROOT}/bin"

    [ -d "$LTPROOT/testcases/bin" ] ||
    {
        echo "FATAL: LTP not installed correctly"
        echo "INFO:  Follow directions in INSTALL!"
        exit 1
    }

    rm -rf ${TMP}/alltests*
}

usage()
{
    cat <<-EOF >&2

    usage: ${0##*/} [options]

    options:
    -f CMDFILES     Execute user defined list of testcases
    -h              Help. Prints all available options.
    -s PATTERN      Only run test cases which match PATTERN.

    example: ${0##*/} -f syscalls -s timerfd


	EOF
exit 0
}

main()
{
    local CMDFILES='syscalls'
    local TAG_RESTRICT_STRING=

    version_date=$(cat "$LTPROOT/Version")

    echo "$version_date"

    while getopts f:hs: arg
    do  case $arg in
        f)  # Execute user defined set of testcases.
            CMDFILES=$OPTARG;;
        h)  usage;;
        s)  TAG_RESTRICT_STRING=$OPTARG;;
        \?) usage;;
        esac
    done

    echo "INFO: Test on files $CMDFILES"

    [ -n "$CMDFILES" ] && \
    {
        #for scenfile in `echo "$CMDFILES" | tr ',' ' '`
        for scenfile in `echo "$CMDFILES"`
        do
            [ -f "$scenfile" ] || scenfile="$LTPROOT/runtest/$scenfile"
            cat "$scenfile" >> ${TMP}/alltests.tmp || \
            {
                echo "FATAL: unable to create command file"
                rm -Rf "$TMP"
                exit 1
            }
        done
    }

    # Skip the lines start with #
    grep -v "^#"  ${TMP}/alltests.tmp > ${TMP}/alltests

    # If enabled, execute only test cases that match the PATTERN
    if [ -n "$TAG_RESTRICT_STRING" ]
    then
        mv -f ${TMP}/alltests ${TMP}/alltests.orig
	    grep $TAG_RESTRICT_STRING ${TMP}/alltests.orig > ${TMP}/alltests
        echo "INFO: Restricted to $TAG_RESTRICT_STRING"    
    fi

    #grep -v "^#" "${TMP}/alltests" | while read -r line
    while read -r line
    do
        # ignore empty lines
        [ "x$line" = x ] && continue

        line_array=($line)
        name=${line_array[0]}
        bin=${line_array[1]}
        len=${#line_array[@]}
        idx=2
        args=()

        echo "INFO: Test case: $name"
        if [ $len -gt 2 ]
        then
            args=${line_array[@]:$idx:$((len - idx))}
        fi
        echo "INFO: ... Commands: $bin $args"

        $LTPROOT/testcases/bin/$bin
    done < "${TMP}/alltests"
}

setup
main "$@"
exit 0
