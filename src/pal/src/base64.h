#ifndef __BASE64_H__
#define __BASE64_H__

#ifdef __cplusplus
extern "C" {
#endif

void base64_decode(const char *b64input, unsigned char *dest, size_t dest_len);

#ifdef __cplusplus
}
#endif

#endif /* __BASE64_H__ */