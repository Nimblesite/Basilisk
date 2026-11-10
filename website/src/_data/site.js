// Implements [WITHDRAWAL-COPY]. Site metadata is derived from the generated
// withdrawal copy rather than restated here, so the title, meta description,
// and social card can never say something the messaging spec does not.
import withdrawal from "./withdrawal.json" with { type: "json" };

export default {
  name: "Basilisk",
  title: withdrawal.title,
  description: withdrawal.line,
  url: "https://www.basilisk-python.dev",
  themeColor: "#e8500a",
  stylesheet: "/assets/css/styles.css",
  github: "https://github.com/Nimblesite/Basilisk",
  organization: {
    name: "Basilisk",
    url: "https://www.basilisk-python.dev",
    logo: "/assets/images/favicon.png",
    sameAs: ["https://github.com/Nimblesite/Basilisk"],
  },
};
