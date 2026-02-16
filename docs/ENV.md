# Configuring Riku via ENV

You can configure deployment settings by placing special variables in an `ENV` file deployed with your app.

## Runtime Settings

* `PIKU_AUTO_RESTART` (boolean, defaults to `true`): Riku will restart all workers every time the app is deployed. You can set it to `0`/`false` if you prefer to deploy first and then restart your workers separately.

### Python

* `PYTHON_VERSION`: Currently ignored - Riku assumes Python 3

### Node

* `NODE_VERSION`: Installs a particular version of Node.js for your app if `nodeenv` is found on the path. Optional; if not specified, the system-wide Node.js package is used.
* `NODE_PACKAGE_MANAGER`: Use an alternate package manager (e.g., set to `yarn` or `pnpm`). The package manager will be installed with `npm install -g` if needed.

> **NOTE**: You will need to stop and re-deploy the app to change the `NODE_VERSION` in a running app.

## Network Settings

* `BIND_ADDRESS`: IP address to which your app will bind (typically `127.0.0.1`)
* `PORT`: TCP port for your app to listen on (auto-assigned if not set)
* `DISABLE_IPV6` (boolean): If set to `true`, it will remove IPv6-specific items from the `nginx` config, which will accept only IPv4 connections

## Riku Worker Settings

These settings control the Riku process supervisor (which replaces uWSGI):

* `RIKU_WORKER_TIMEOUT` (integer, defaults to `7200`): Kill unresponsive workers after N seconds
* `RIKU_WORKER_GRACE_PERIOD` (integer, defaults to `30`): Time to wait before forcefully killing workers on shutdown
* `RIKU_MAX_RESTARTS` (integer, defaults to `5`): Maximum restart attempts before marking app as failed

> **NOTE**: For scaling workers, use the `SCALING` file instead of environment variables. Example: `web=4` runs 4 web workers.

## `nginx` Settings

* `NGINX_SERVER_NAME`: Set the virtual host name associated with your app
* `NGINX_STATIC_PATHS` (string, comma separated list): Set an array of `/url:path` values that will be served directly by `nginx`
* `NGINX_CLOUDFLARE_ACL` (boolean, defaults to `false`): Activate an ACL allowing access only from Cloudflare IPs
* `NGINX_HTTPS_ONLY` (boolean, defaults to `false`): Tell `nginx` to auto-redirect non-SSL traffic to SSL site
* `NGINX_INCLUDE_FILE`: A file in the app's dir to include in nginx config `server` section - useful for including custom `nginx` directives
* `NGINX_ALLOW_GIT_FOLDERS` (boolean, defaults to `false`): Allow access to `.git` folders
* `NGINX_CATCH_ALL` (string, defaults to `""`): Specifies a filename to serve to all requests regardless of path (useful when using client-side routing)

> **NOTE**: If used with Cloudflare, `NGINX_HTTPS_ONLY` will cause an infinite redirect loop - keep it set to `false`, use `NGINX_CLOUDFLARE_ACL` instead and add a Cloudflare Page Rule to "Always Use HTTPS" for your server (use `domain.name/*` to match all URLs).

### `nginx` Caching

When `NGINX_CACHE_PREFIXES` is set, `nginx` will cache requests for those URL prefixes to the running application and reply on its own for `NGINX_CACHE_TIME` to the outside. This is meant to be used for compute-intensive operations like resizing images or providing large chunks of data that change infrequently (like a sitemap).

The behavior of the cache can be controlled with the following variables:

* `NGINX_CACHE_PREFIXES` (string, comma separated list): Set an array of `/url` values that will be cached by `nginx`
* `NGINX_CACHE_SIZE` (integer, defaults to 1): Set the maximum size of the `nginx` cache, in GB
* `NGINX_CACHE_TIME` (integer, defaults to 3600): Set the amount of time (in seconds) that valid backend replies (`200 304`) will be cached
* `NGINX_CACHE_REDIRECTS` (integer, defaults to 3600): Set the amount of time (in seconds) that backend redirects (`301 307`) will be cached
* `NGINX_CACHE_ANY` (integer, defaults to 3600): Set the amount of time (in seconds) that any other replies (other than errors) will be cached
* `NGINX_CACHE_CONTROL` (integer, defaults to 3600): Set the amount of time (in seconds) for cache control headers (`Cache-Control "public, max-age=3600"`)
* `NGINX_CACHE_EXPIRY` (integer, defaults to 86400): Set the amount of time (in seconds) that cache entries will be kept on disk
* `NGINX_CACHE_PATH` (string, defaults to `~/.riku/cache/<appname>`): Location for the `nginx` cache data

> **NOTE**: `NGINX_CACHE_PATH` will be _completely managed by `nginx` and cannot be removed by Riku when the application is destroyed_. This is because `nginx` sets the ownership for the cache to be exclusive to itself. You will either need to clean it up manually after destroying the app or store it in a temporary filesystem.

Cache revalidation is not currently supported - `nginx` will only ask your backend for new content when `NGINX_CACHE_TIME` elapses. If you require that kind of behavior, use `NGINX_INCLUDE_FILE` to add custom cache configuration.

Also, keep in mind that using `nginx` caching with a `static` website worker will _not_ work (and there's no point to it either).

## Acme Settings

* `ACME_ROOT`: Directory for ACME/Let's Encrypt client (default: `~/.acme.sh`)
* `ACME_ROOT_CA`: Set the certificate authority that Acme should use to generate public SSL certificates (string, default: `letsencrypt.org`)

## Deprecated (uWSGI-specific - Not Used in Riku)

The following environment variables are **NOT used** in Riku as it uses a custom process supervisor instead of uWSGI:

- `UWSGI_MAX_REQUESTS`
- `UWSGI_LISTEN`
- `UWSGI_PROCESSES`
- `UWSGI_ENABLE_THREADS`
- `UWSGI_LOG_MAXSIZE`
- `UWSGI_LOG_X_FORWARDED_FOR`
- `UWSGI_GEVENT`
- `UWSGI_ASYNCIO`
- `UWSGI_INCLUDE_FILE`
- `UWSGI_IDLE`

For worker scaling, use the `SCALING` file instead. Example:
```
web=4
worker=2
```
