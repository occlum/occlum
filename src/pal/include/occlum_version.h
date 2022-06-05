// This file generated by update_version.sh
// Do not update this file manually.
#ifndef _OCCLUM_VERSION_H_
#define _OCCLUM_VERSION_H_

// Version = 0.27.3
#define OCCLUM_MAJOR_VERSION    0
#define OCCLUM_MINOR_VERSION    27
#define OCCLUM_PATCH_VERSION    3

#define STRINGIZE_PRE(X) #X
#define STRINGIZE(X) STRINGIZE_PRE(X)

#define OCCLUM_VERSION_NUM_STR STRINGIZE(OCCLUM_MAJOR_VERSION) "." \
                    STRINGIZE(OCCLUM_MAJOR_VERSION) "." STRINGIZE(OCCLUM_PATCH_VERSION)

#endif
