import { useEffect, useState } from "react";

export default function App() {
  const [health, setHealth] = useState("loading...");

  useEffect(() => {
    fetch("/api/health")
      .then((r) => r.text())
      .then(setHealth)
      .catch((e) => setHealth(String(e)));
  }, []);

  return (
    <main style={{ fontFamily: "system-ui, sans-serif", maxWidth: "40rem", margin: "2rem auto", padding: "0 1rem" }}>
      <h1>KOL Fight</h1>
      <pre>{health}</pre>
    </main>
  );
}
