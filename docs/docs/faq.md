# FAQ - Riku

## General Questions

**Q: Why Riku?**

**A:** Riku is a complete Rust port of the original [Piku](https://github.com/piku/piku) micro-PaaS. The name honors the original project while signifying our independent implementation in Rust.

**Q: Why Rust?**

**A:** Rust provides several advantages over the original Python implementation:
- **Single binary deployment** - No Python runtime dependencies
- **Memory safety** - No garbage collection, no runtime errors
- **Performance** - Compiled code runs faster with lower memory footprint
- **Type safety** - Catch errors at compile time instead of runtime
- **Low resource usage** - Ideal for small VPS and edge deployments

**Q: Is Riku compatible with Piku?**

**A:** Yes. Riku maintains compatibility with Piku's:
- Directory structure (`~/.riku/` instead of `~/.piku/`)
- Environment variable format
- Procfile syntax
- Plugin system
- Git-based deployment workflow

**Q: Does Riku use uWSGI?**

**A:** No. Riku implements its own process supervisor in Rust, replacing uWSGI Emperor. This gives us:
- Better control over process lifecycle
- Native health checking
- Built-in log rotation
- Lower memory footprint
- No Python dependencies

**Q: What runtimes does Riku support?**

**A:** Riku supports:
- Python (requirements.txt, Poetry, uv)
- Node.js (built with nub: reads npm/pnpm/bun lockfiles)
- Ruby (Gemfile)
- Go (modules, Godeps)
- Java (Maven, Gradle)
- Clojure (CLI, Leiningen)
- Rust (Cargo)
- Static sites
- Containers (Docker/Podman)

## Deployment

**Q: How do I deploy an app?**

**A:** Just like Heroku or Piku:
```bash
git remote add riku riku@edge.example:myapp
git push riku main
```

**Q: How do I scale my app?**

**A:** Use the `SCALING` file:
```bash
echo "web=4" > SCALING
echo "worker=2" >> SCALING
git add SCALING && git commit -m "scale"
git push riku main
```

Or use the CLI:
```bash
riku ps myapp --scale web=4 worker=2
```

**Q: How do I set environment variables?**

**A:** Use the config command:
```bash
riku config set myapp KEY=value ANOTHER_KEY=value2
```

Or create an `ENV` file in `~/.riku/envs/myapp/ENV`.

**Q: Can I use custom nginx configuration?**

**A:** No. An `nginx.conf`, `nginx.custom.conf`, `nginx.custom` or `.nginx.conf` in your app directory is not supported: riku ignores it, prints a warning at deploy, and uses the generated config. There is no snippet file either. The route covers domains, backends, TLS, static files and the HTTP mode. Set headers and caching in the app.

**Q: Can I store my bare git repo in a custom location?**

**A:** Yes. Riku automatically symlinks custom repo locations to `~/.riku/repos/`:

```bash
# On server: create bare repo anywhere
git init --bare ~/my-projects/myapp.git

# Push to custom path
git remote add riku riku@server:~/my-projects/myapp.git
git push riku main

# Riku creates: ~/.riku/repos/myapp.git → ~/my-projects/myapp.git
```

**Q: Does Riku auto-start the supervisor?**

**A:** Yes. On first deployment, Riku automatically starts the supervisor daemon if not running. Nginx site files are written by the router helper to `/etc/riku/nginx`, which nginx loads through `/etc/nginx/conf.d/riku.conf` (both set up by `sudo riku init`).

**Q: Do I need to install python3-venv for Python apps?**

**A:** Yes, on Debian/Ubuntu servers, install the package:
```bash
sudo apt install python3-venv python3-full
```

This is required for creating Python virtual environments.

## Troubleshooting

**Q: My app won't start. How do I debug?**

**A:** Check the logs:
```bash
riku logs myapp
riku logs myapp web
```

**Q: How do I restart my app?**

**A:** 
```bash
riku restart myapp
```

This restarts the running workers from the current build. A new build
starts with `git push` or `riku deploy myapp`.

**Q: Where are logs stored?**

**A:** In `~/.riku/logs/<app>/`. Logs are automatically rotated when they exceed 10MB.

**Q: How do I backup my apps?**

**A:** Use the backup plugin or manually:
```bash
tar -czf backup.tar.gz ~/.riku/apps/myapp
```

## Advanced

**Q: Can I run multiple apps on the same domain?**

**A:** Yes, use different subdomains or paths with `RIKU_DOMAINS` and `RIKU_NGINX_STATIC_PATHS`.

**Q: How do I enable HTTPS?**

**A:** Set the domain and enable HTTPS redirect:
```bash
riku settings set myapp routing.domains=example.com routing.https_only=true
```

Then obtain SSL certificates (ACME support is built-in).

**Q: Can I use Riku with Cloudflare?**

**A:** Yes, as a plain proxy in front of your domain. Riku has no Cloudflare IP allow list. If you want to limit direct access to the origin, do it in a firewall.

**Q: How do I add custom cron jobs?**

**A:** Add to your Procfile:
```
cron: 0 2 * * * /path/to/script.sh
```

**Q: Does Riku support zero-downtime deployments?**

**A:** For a worker with a health check, yes: a deploy or `riku restart myapp` starts the new copy next to the old one and swaps it in once it passes its probe. A worker without a health check is stopped and started again, with a brief gap. Container apps are replaced one copy at a time. `riku restart` never rebuilds; `riku deploy` does.

## Migration from Piku

**Q: How do I migrate from Piku to Riku?**

**A:** 
1. Install Riku alongside Piku
2. Riku uses `~/.riku/` instead of `~/.piku/`
3. Copy your apps: `cp -r ~/.piku/apps/* ~/.riku/apps/`
4. Copy your env files: `cp -r ~/.piku/envs/* ~/.riku/envs/`
5. Update git remotes to point to Riku
6. Test each app and migrate gradually

**Q: Are Piku plugins compatible with Riku?**

**A:** Yes. Shell-based plugins work with both. Place them in `~/.riku/plugins/`.

**Q: What about uWSGI configuration?**

**A:** Riku doesn't use uWSGI. Replace uWSGI-specific env vars with Riku equivalents:
- `UWSGI_PROCESSES` → Use the `SCALING` file or `riku ps myapp --scale web=2`
- `UWSGI_MAX_REQUESTS` → Not needed (handled by supervisor)
- `UWSGI_*` → See [docs/docs/env.md](env.md) for the full variable reference

## Performance

**Q: What's the memory footprint?**

**A:** Typical usage:
- Riku supervisor: 10-30 MB
- Riku binary: ~8 MB
- Per app process: 10-200 MB (depends on runtime)
- Nginx: 5-15 MB

**Q: How many apps can I run?**

**A:** Depends on your server resources. On a 512MB VPS, you can comfortably run 2-3 small apps. On 2GB, 10+ apps is feasible.

## Support

**Q: Where do I get help?**

**A:** 
- Open an issue on GitHub
- Check the documentation in `docs/`
- Review example apps in `examples/`

**Q: How do I contribute?**

**A:** 
1. Fork the repository
2. Create a feature branch
3. Add tests for new features
4. Submit a pull request

See `CONTRIBUTING.md` for details.
