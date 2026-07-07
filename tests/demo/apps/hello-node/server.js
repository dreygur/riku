const http = require("http");

const port = process.env.PORT || 8080;
http
  .createServer((req, res) => {
    res.writeHead(200, { "Content-Type": "text/plain" });
    res.end(`hello from hello-node (pid ${process.pid})\n`);
  })
  .listen(port, "0.0.0.0", () => console.log(`hello-node listening on ${port}`));
