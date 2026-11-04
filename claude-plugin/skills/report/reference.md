# Report components

What the chrome styles inside `<article>`. Headings `h2`/`h3` are numbered
by CSS counters and listed in the contents; `h4` is a small caps label.
Everything else is plain HTML with these classes.

```html
<p class="lead">The question in one sentence. The answer in the next.</p>

<div class="tiles">
  <div class="tile">
    <span class="label">Best pick</span
    ><span class="value">Dyson Supersonic</span
    ><span class="delta">12 990 Kč</span>
  </div>
  <div class="tile">
    <span class="label">Candidates</span><span class="value">14</span
    ><span class="delta up">+3 since October</span>
  </div>
  <div class="tile">
    <span class="label">Noise</span><span class="value">78 dB</span
    ><span class="delta down">quietest in class</span>
  </div>
</div>

<div class="callout warn">
  <span class="k">Caveat</span>
  <p>Black Friday prices; checked 2026-11-02 at 19:00.</p>
</div>
<!-- .callout.info / .good / .warn / .bad -->

<h2>What to buy</h2>
<p>
  Prose. <span class="badge good">recommended</span>
  <span class="badge warn">noisy</span> <span class="badge bad">avoid</span>
  <span class="badge">neutral</span>
</p>

<div class="table-wrap">
  <table>
    <thead>
      <tr>
        <th>Model</th>
        <th class="num">Price</th>
        <th class="num">Watt</th>
        <th>Verdict</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td>Dyson Supersonic</td>
        <td class="num">12 990</td>
        <td class="num">1 600</td>
        <td>best</td>
      </tr>
      <tr class="total">
        <td>Median</td>
        <td class="num">2 490</td>
        <td class="num">2 100</td>
        <td></td>
      </tr>
    </tbody>
    <caption>
      Prices in CZK, Alza.cz, 2026-11-02.
    </caption>
  </table>
</div>

<figure>
  <svg viewBox="0 0 600 240" role="img" aria-label="Price by model">
    <!-- bars: fill="var(--series-1)"; axes: stroke="var(--border-strong)"; labels: fill="var(--muted)" -->
  </svg>
  <figcaption>Price by model; the dashed line is the median.</figcaption>
  <ul class="legend">
    <li><i style="background:var(--series-1)"></i>Price</li>
    <li><i style="background:var(--series-2)"></i>Median</li>
  </ul>
</figure>

<div class="grid-2">
  <div>
    <h3>Pros</h3>
    <ul>
      <li>…</li>
    </ul>
  </div>
  <div>
    <h3>Cons</h3>
    <ul>
      <li>…</li>
    </ul>
  </div>
</div>

<dl class="kv">
  <dt>Checked</dt>
  <dd>2026-11-02</dd>
  <dt>Source</dt>
  <dd>Alza.cz, logged-in prices</dd>
</dl>

<div class="bar" aria-label="Share">
  <span style="width:62%"></span
  ><span style="width:25%;background:var(--series-2)"></span>
</div>

<details>
  <summary>The full candidate list (14)</summary>
  <p>…</p>
</details>

<h2>Sources</h2>
<ul class="sources">
  <li><a href="https://www.alza.cz/…">Alza.cz listing</a>, 2026-11-02</li>
</ul>
```

Also: `blockquote`, `code`, `pre`, `hr`, `mark`, `kbd`, `abbr`, `.muted`,
`.small`, an `<img src="data:image/png;base64,…">`. External links open in
a new tab by themselves. Headings get ids from their text; set your own
`id` when a link to the section must survive a reworded heading.

Theme variables for charts: `--series-1` … `--series-6` (categorical),
`--seq-1` … `--seq-6` (one-hue ramp, light to dark), `--good` `--warn`
`--bad` `--info`, `--fg` `--muted` `--border` `--border-strong` `--surface`
`--surface-2` `--accent`. Fonts: `var(--font)`, `var(--mono)`.
