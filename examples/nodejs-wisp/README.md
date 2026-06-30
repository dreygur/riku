# Node.js + Wisp Sample Application

This example demonstrates using `wisp`, a 3rd party npm installed binary to run scripts.

It is otherwise identical to the node example.

> **Note:** Unlike the other node examples, this one is **not** on
> [nub](https://nubjs.com). `wisp@0.11.2` pulls `escodegen` through an exotic
> `git://` specifier, which nub blocks by default (`blockExoticSubdeps`) for
> supply-chain safety. This example therefore stays on npm — deploy it to a
> server that still has npm available, or replace the `wisp` dependency.

To publish this app to Riku, make a copy of this folder and run the following commands:

```bash
git init .
git remote add riku deploy@your_server:wispchat
git add .
git commit -a -m "initial commit"
git push riku main
```

Then you can set up an SSL cert and connect a domain by setting config variables like this:

```bash
riku config set wispchat NGINX_SERVER_NAME=your_server NGINX_HTTPS_ONLY=1
```

Then visit the site `your_server` and you will see a simple websocket chat application.
