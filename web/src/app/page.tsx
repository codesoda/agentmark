import { SiteFooter } from "@/components/site/site-footer";
import { SiteHeader } from "@/components/site/site-header";
import { Agents, Bundle, Capture, Hero, Install, Local, Problem, Setup, Writeup } from "@/components/site/home-sections";

export default function HomePage() {
  return (
    <>
      <SiteHeader />
      <main id="main">
        <Hero />
        <Problem />
        <Capture />
        <Bundle />
        <Agents />
        <Writeup />
        <Local />
        <Setup />
        <Install />
      </main>
      <SiteFooter />
    </>
  );
}
