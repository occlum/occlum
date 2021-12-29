/*
 * Base64 encoding/decoding (RFC1341)
 * Copyright (c) 2005-2011, Jouni Malinen <j@w1.fi>
 *
 * This software may be distributed under the terms of the BSD license.
 * See README for more details.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "pal_log.h"
#include "base64.h"


static const unsigned char base64_table[65] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

static size_t base64_decode_len(const char *b64input) {
    size_t len = strlen(b64input), padding = 0;

    if (b64input[len - 1] == '=' && b64input[len - 2] == '=') { //last two chars are =
        padding = 2;
    } else if (b64input[len - 1] == '=') { //last char is =
        padding = 1;
    }

    return (len * 3) / 4 - padding;
}

/**
 * base64_decode - Base64 decode
 */
void base64_decode(const char *b64input, unsigned char *dest, size_t dest_len) {
    unsigned char dtable[256], *pos, block[4], tmp;
    size_t i, count, olen;
    size_t len = strlen(b64input);

    memset(dtable, 0x80, 256);
    for (i = 0; i < sizeof(base64_table) - 1; i++) {
        dtable[base64_table[i]] = (unsigned char) i;
    }
    dtable['='] = 0;

    olen = base64_decode_len(b64input);
    if (olen > dest_len) {
        PAL_WARN("Base64 encoded length %ld is biggeer than %ld\n", olen, dest_len);
        return;
    }

    pos = dest;
    count = 0;
    for (i = 0; i < len; i++) {
        tmp = dtable[(unsigned char)b64input[i]];
        if (tmp == 0x80) {
            continue;
        }
        block[count] = tmp;
        count++;
        if (count == 4) {
            *pos++ = (block[0] << 2) | (block[1] >> 4);
            *pos++ = (block[1] << 4) | (block[2] >> 2);
            *pos++ = (block[2] << 6) | block[3];
            count = 0;
        }
    }
}