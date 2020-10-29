

#ifndef __RUST_BINDINGS_GEN_H_
#define __RUST_BINDINGS_GEN_H_

/* Generated with cbindgen:0.13.2 */

/* DO NOT EDIT This file is autogenerated by cbindgen. Don't modify this manually. */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct IpcClient IpcClient;

typedef struct SCPacket SCPacket;

uint32_t rs_create_ipc_client(const char *server_name, IpcClient **client);

int64_t rs_ipc_populate_packets(IpcClient *ipc,
                                SCPacket **packets,
                                uint64_t len);

void rs_ipc_release_packet(uint8_t *user);

void rs_release_ipc_client(IpcClient *ipc);

#endif /* __RUST_BINDINGS_GEN_H_ */
