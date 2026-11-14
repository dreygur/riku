---
hide:
  - navigation
  - toc
---

<div class="rk-home">

<section class="rk-sec rk-hero">
  <div class="rk-wrap rk-hero__grid">
    <div>
      <p class="rk-eyebrow">single binary<i></i>git push deploys<i></i>written in rust</p>
      <h1>Deploy with <span class="rk-accent">git push</span>.<br>On a box you own.</h1>
      <p class="rk-tagline">Riku is the smallest PaaS you've ever seen. One Rust binary turns a small server into a Heroku style deploy target, with no runtime to install and no daemons you didn't ask for.</p>
      <div class="rk-cta">
        <a class="rk-btn is-primary" href="quick-start/">Get started</a>
        <a class="rk-btn" href="installation/">Install</a>
        <a class="rk-btn is-ghost" href="cli/">CLI reference</a>
      </div>
    </div>
    <div class="rk-term">
      <div class="rk-term__bar"><i></i><i></i><i></i><span>you@laptop: ~/myapp</span></div>
<pre><span class="ln" style="--i:0"><span class="pr">$</span> <span class="cm">git push riku main</span></span><span class="ln" style="--i:1"><span class="rm">remote:</span> <span class="gl">┌</span>  <span class="bd">Deploying myapp</span>  <span class="dm">54ba34f "Add login"</span></span><span class="ln" style="--i:2"><span class="rm">remote:</span> <span class="gl">│</span></span><span class="ln" style="--i:3"><span class="rm">remote:</span> <span class="ok">◇</span>  Runtime  <span class="dm">node</span></span><span class="ln" style="--i:4"><span class="rm">remote:</span> <span class="ok">◇</span>  Build  <span class="dm">6.2s</span></span><span class="ln" style="--i:5"><span class="rm">remote:</span> <span class="ok">◇</span>  Processes  <span class="dm">web 1</span></span><span class="ln" style="--i:6"><span class="rm">remote:</span> <span class="ok">◇</span>  Route  <span class="dm">caddy</span></span><span class="ln" style="--i:7"><span class="rm">remote:</span> <span class="gl">│</span></span><span class="ln" style="--i:8"><span class="rm">remote:</span> <span class="gl">└</span>  <span class="bd">Deployed in 11.4s</span></span><span class="ln" style="--i:9"><span class="rm">remote:</span></span><span class="ln" style="--i:10"><span class="rm">remote:</span>    <span class="bd">URL</span>       <span class="ur">https://myapp.example.com</span></span><span class="ln" style="--i:11"><span class="rm">remote:</span>    <span class="bd">Logs</span>      rk logs myapp <span class="cur"></span></span></pre>
    </div>
  </div>
</section>

<section class="rk-sec">
  <div class="rk-wrap rk-pad rk-reveal">
    <p class="rk-kick"><b>//</b> what it looks like running</p>
    <h2 class="rk-h2">A dashboard baked into the binary.</h2>
    <p class="rk-sub"><code>riku dashboard</code> serves it straight from the riku binary. Nothing else to install, no Node on the server.</p>
    <div class="rk-app">
      <div class="rk-app__head">
        <div>
          <h3>myapp</h3>
          <div class="rk-meta"><span>node</span><span>caddy</span><span><b>myapp.example.com</b></span><span>2h uptime</span></div>
        </div>
        <div class="rk-actions">
          <span class="rk-badge"><i></i>running</span>
          <span class="rk-btn is-ghost">Logs</span>
          <span class="rk-btn">Restart</span>
        </div>
      </div>
      <div class="rk-rows">
        <div class="rk-row is-head"><span>Worker</span><span>State</span><span>Memory</span><span>CPU</span></div>
        <div class="rk-row"><span>web.1</span><span class="rk-state"><i></i>alive</span><span class="rk-mem"><span class="rk-meter"><i style="width:34%"></i></span>44 MB</span><span>0.4%</span></div>
        <div class="rk-row"><span>worker.1</span><span class="rk-state"><i></i>alive</span><span class="rk-mem"><span class="rk-meter"><i style="width:22%"></i></span>29 MB</span><span>0.1%</span></div>
      </div>
    </div>
    <a class="rk-more" href="dashboard/">Read about the dashboard &rarr;</a>
  </div>
</section>

<section class="rk-sec">
  <div class="rk-wrap">
    <div class="rk-nums rk-reveal">
      <div class="rk-num"><b>4.4<small>MB</small></b><span>Release download</span></div>
      <div class="rk-num"><b>256<small>MB</small></b><span>Minimum host RAM</span></div>
      <div class="rk-num"><b>7</b><span>Runtimes, plus containers</span></div>
      <div class="rk-num"><b>1</b><span>Binary on the server</span></div>
    </div>
  </div>
</section>

<section class="rk-sec">
  <div class="rk-wrap rk-pad rk-reveal">
    <p class="rk-kick"><b>//</b> deploy in three steps</p>
    <h2 class="rk-h2">From empty server to live app.</h2>
    <div class="rk-steps">
      <div class="rk-step">
        <span class="rk-n">1</span>
        <h3>Initialize the server</h3>
        <p>Install the binary, then run this once as the deploy user.</p>
        <div class="rk-cmd"><span class="rk-cmd__line"><span class="rk-cmd__pr">$</span><span class="rk-cmd__text">riku init</span></span><button class="rk-cmd__copy" type="button">copy</button></div>
      </div>
      <div class="rk-step">
        <span class="rk-n">2</span>
        <h3>Add a Procfile</h3>
        <p>Name your processes. That's the whole config.</p>
        <div class="rk-cmd"><span class="rk-cmd__line"><span class="rk-cmd__text">web: node server.js</span></span><button class="rk-cmd__copy" type="button">copy</button></div>
      </div>
      <div class="rk-step">
        <span class="rk-n">3</span>
        <h3>Push</h3>
        <p>Riku builds the app, supervises its processes and routes traffic to it.</p>
        <div class="rk-cmd"><span class="rk-cmd__line"><span class="rk-cmd__pr">$</span><span class="rk-cmd__text">git push riku main</span></span><button class="rk-cmd__copy" type="button">copy</button></div>
      </div>
    </div>
  </div>
</section>

<section class="rk-sec">
  <div class="rk-wrap rk-pad rk-reveal">
    <p class="rk-kick"><b>//</b> runtimes</p>
    <h2 class="rk-h2">Detects your stack, no config.</h2>
    <div class="rk-tags">
      <a class="rk-tag" href="runtimes/#python"><i></i>python</a>
      <a class="rk-tag" href="runtimes/#nodejs"><i></i>node</a>
      <a class="rk-tag" href="runtimes/#ruby"><i></i>ruby</a>
      <a class="rk-tag" href="runtimes/#go"><i></i>go</a>
      <a class="rk-tag" href="runtimes/#java"><i></i>java</a>
      <a class="rk-tag" href="runtimes/#clojure"><i></i>clojure</a>
      <a class="rk-tag" href="runtimes/#rust"><i></i>rust</a>
      <a class="rk-tag" href="runtimes/#container"><i></i>containers</a>
    </div>
  </div>
</section>

<section class="rk-sec">
  <div class="rk-wrap rk-pad rk-reveal">
    <p class="rk-kick"><b>//</b> why riku</p>
    <h2 class="rk-h2">Smaller than Dokku and CapRover.</h2>
    <div class="rk-cmp">
      <div class="rk-cmp__row is-head"><span></span><span>Riku</span><span>Dokku / CapRover</span></div>
      <div class="rk-cmp__row"><span>Install</span><span class="rk-yes">One binary</span><span>Docker based platform</span></div>
      <div class="rk-cmp__row"><span>Minimum RAM</span><span class="rk-yes">256 MB</span><span>1 GB</span></div>
      <div class="rk-cmp__row"><span>Procfile apps</span><span class="rk-yes">Plain supervised processes</span><span>Docker containers</span></div>
    </div>
  </div>
</section>

<section class="rk-sec" style="border-bottom:0">
  <div class="rk-wrap rk-pad rk-reveal">
    <div class="rk-panel">
      <span class="rk-panel__ico"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" aria-hidden="true"><path d="M12 4v15"/><path d="M6 13l6 6 6-6"/></svg></span>
      <h2>Deploy your first app.</h2>
      <p>Install riku on the server and the rk client on your laptop, then push.</p>
      <div class="rk-cmds">
        <div class="rk-cmd"><span class="rk-cmd__line"><span class="rk-cmd__tag">server</span><span class="rk-cmd__pr">$</span><span class="rk-cmd__text">curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sudo sh</span></span><button class="rk-cmd__copy" type="button">copy</button></div>
        <div class="rk-cmd"><span class="rk-cmd__line"><span class="rk-cmd__tag">laptop</span><span class="rk-cmd__pr">$</span><span class="rk-cmd__text">curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sh -s -- --rk</span></span><button class="rk-cmd__copy" type="button">copy</button></div>
      </div>
      <a class="rk-btn is-primary" href="quick-start/">Read the quick start</a>
    </div>
  </div>
</section>

</div>
