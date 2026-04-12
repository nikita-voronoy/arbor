#include "types.h"
#include <stdlib.h>
#include <string.h>

static struct Connection *find_connection(struct Server *srv, int fd) {
    for (int i = 0; i < srv->active_count; i++) {
        if (srv->connections[i].fd == fd)
            return &srv->connections[i];
    }
    return NULL;
}

static void close_connection(struct Connection *conn) {
    conn->status = STATUS_ERROR;
    conn->fd = -1;
}

struct Server *server_create(int max_conn) {
    struct Server *srv = malloc(sizeof(struct Server));
    srv->connections = calloc(max_conn, sizeof(struct Connection));
    srv->max_connections = max_conn;
    srv->active_count = 0;
    return srv;
}

int server_accept(struct Server *srv, const char *host, int port) {
    if (srv->active_count >= srv->max_connections)
        return -1;

    struct Connection *conn = &srv->connections[srv->active_count++];
    conn->fd = srv->active_count;
    conn->host = strdup(host);
    conn->port = CLAMP(port, 1, 65535);
    conn->status = STATUS_OK;
    return conn->fd;
}

void server_disconnect(struct Server *srv, int fd) {
    struct Connection *conn = find_connection(srv, fd);
    if (conn)
        close_connection(conn);
}

enum Status server_status(struct Server *srv, int fd) {
    struct Connection *conn = find_connection(srv, fd);
    return conn ? conn->status : STATUS_ERROR;
}

void server_destroy(struct Server *srv) {
    for (int i = 0; i < srv->active_count; i++) {
        free(srv->connections[i].host);
    }
    free(srv->connections);
    free(srv);
}
