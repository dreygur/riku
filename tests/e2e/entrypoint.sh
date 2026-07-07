#!/bin/bash

# Start nginx
/usr/sbin/nginx

# Start SSH daemon
exec /usr/sbin/sshd -D
