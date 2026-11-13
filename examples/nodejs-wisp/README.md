# Node.js + Wisp Sample Application

This example demonstrates using `wisp`, a 3rd party npm installed binary to run scripts.

It is otherwise identical to the node example.

> Unlike the other node examples, this one is **not** on
> [nub](https://nubjs.com). `wisp@0.11.2` pulls `escodegen` through an exotic
> `git://` specifier, which nub blocks by default (`blockExoticSubdeps`) for
> supply-chain safety. This example therefore stays on npm, deploy it to a
> server that still has npm available, or replace the `wisp` dependency.

To publish this app to Riku, make a copy of this folder and run the following commands:

```bash
git init .
git remote add riku riku@edge.example:wispchat
git add .
git commit -a -m "initial commit"
git push riku main
```

Then you can set up an SSL cert and connect a domain with riku settings like this:

```bash
riku settings set wispchat routing.domains=edge.example routing.https_only=true
```

Then visit `https://edge.example` and you will see a simple websocket chat application.
