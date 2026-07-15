const root = document.documentElement;
const themeToggle = document.querySelector("#theme-toggle");
const storedTheme = localStorage.getItem("claudemeter-site-theme");
const preferredTheme = window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";

root.dataset.theme = storedTheme || preferredTheme;

themeToggle?.addEventListener("click", () => {
  const nextTheme = root.dataset.theme === "dark" ? "light" : "dark";
  root.dataset.theme = nextTheme;
  localStorage.setItem("claudemeter-site-theme", nextTheme);
});

const revealObserver = new IntersectionObserver(
  entries => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        entry.target.classList.add("is-visible");
        revealObserver.unobserve(entry.target);
      }
    }
  },
  { threshold: 0.12 }
);

document.querySelectorAll(".reveal").forEach(element => revealObserver.observe(element));

fetch("metrics.json")
  .then(response => {
    if (!response.ok) throw new Error("Project metrics unavailable");
    return response.json();
  })
  .then(metrics => {
    const starsValue = document.querySelector("#stars-value");
    const issuesValue = document.querySelector("#issues-value");
    const releaseValue = document.querySelector("#release-value");
    const releaseChip = document.querySelector("#release-chip-text");

    if (starsValue) starsValue.textContent = metrics.stars.toLocaleString();
    if (issuesValue) issuesValue.textContent = metrics.openIssues.toLocaleString();
    if (releaseValue) releaseValue.textContent = metrics.latestRelease;
    if (releaseChip) releaseChip.textContent = `ClaudeMeter ${metrics.latestRelease} is available`;
  })
  .catch(() => {});
