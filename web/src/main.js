import "./styles.css";

const app = document.querySelector("#app");

app.innerHTML = `
  <main class="shell">
    <section class="panel">
      <p class="eyebrow">Desktop Shell</p>
      <h1>Uniseq</h1>
      <p class="body">
        The Tauri host is scaffolded. Backend session and command wiring live in Rust.
        React product work can plug into this shell later.
      </p>
      <dl class="status">
        <div>
          <dt>Frontend</dt>
          <dd>Placeholder web shell</dd>
        </div>
        <div>
          <dt>Backend</dt>
          <dd>Embedded Rust crate</dd>
        </div>
        <div>
          <dt>Bridge</dt>
          <dd>Tauri commands + DTOs</dd>
        </div>
      </dl>
    </section>
  </main>
`;
