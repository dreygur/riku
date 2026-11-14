// Home page behavior: copy buttons on command rows and the scroll reveal.
// Material's instant navigation swaps pages without a reload, so this runs
// on every document$ emission and returns early off the home page. Without
// JavaScript the page stays fully visible: sections only start hidden once
// .rk-motion is set here.

const LABEL_RESET_MS = 1500;
const REVEAL_MARGIN = "0px 0px -40px 0px";

function flashLabel(button, label) {
  button.textContent = label;
  setTimeout(() => {
    button.textContent = "copy";
  }, LABEL_RESET_MS);
}

function copyCommand(button) {
  const text = button.parentElement.querySelector(".rk-cmd__text").textContent;
  navigator.clipboard.writeText(text).then(
    () => flashLabel(button, "copied"),
    () => flashLabel(button, "failed"),
  );
}

function wireCopy(home) {
  home.querySelectorAll(".rk-cmd__copy").forEach((button) => {
    button.addEventListener("click", () => copyCommand(button));
  });
}

function wireReveal(home) {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
    return;
  }

  const observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) {
        continue;
      }
      entry.target.classList.add("is-in");
      observer.unobserve(entry.target);
    }
  }, { rootMargin: REVEAL_MARGIN });

  home.classList.add("rk-motion");
  home.querySelectorAll(".rk-reveal").forEach((section) => observer.observe(section));
}

document$.subscribe(() => {
  const home = document.querySelector(".rk-home");
  if (!home) {
    return;
  }

  wireCopy(home);
  wireReveal(home);
});
