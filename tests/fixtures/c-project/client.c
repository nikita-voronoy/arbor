#include "types.h"

struct Client {
    struct Connection conn;
    char *username;
};

struct Client *client_create(const char *host, int port, const char *user) {
    struct Client *c = malloc(sizeof(struct Client));
    c->conn.host = strdup(host);
    c->conn.port = port;
    c->conn.status = STATUS_PENDING;
    c->username = strdup(user);
    return c;
}

enum Status client_connect(struct Client *c) {
    c->conn.status = STATUS_OK;
    return c->conn.status;
}

void client_destroy(struct Client *c) {
    free(c->conn.host);
    free(c->username);
    free(c);
}
