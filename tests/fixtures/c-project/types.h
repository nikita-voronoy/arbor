#ifndef TYPES_H
#define TYPES_H

#define MAX_BUFFER_SIZE 4096
#define VKD3D_FLAG_STAGGER 0x1
#define CLAMP(x, lo, hi) ((x) < (lo) ? (lo) : (x) > (hi) ? (hi) : (x))

typedef unsigned int uint32_t;

enum Status {
    STATUS_OK,
    STATUS_ERROR,
    STATUS_PENDING,
    STATUS_TIMEOUT = 255,
};

struct Connection {
    int fd;
    char *host;
    int port;
    enum Status status;
};

struct Server {
    struct Connection *connections;
    int max_connections;
    int active_count;
};

/* Forward declaration — should NOT be indexed as definition */
struct Client;

#endif
