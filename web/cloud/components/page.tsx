import type { ReactNode } from "react";

import { Brand, CenteredPage, Footer } from "@orchester/ui/components/frame";

import { t } from "@/lib/i18n";

export function CloudBrand() {
  return <Brand name={t("brand.name")} product={t("brand.product")} />;
}

export function CloudFooter() {
  const aside = (
    <>
      <span>{t("footer.terms")}</span>
      <span>{t("footer.privacy")}</span>
    </>
  );
  return <Footer promise={t("footer.promise")} aside={aside} />;
}

/** The Cloud's form pages (sign-up, sign-in, links): the brand, one card, the footer. */
export function Shell({ children, below }: { children: ReactNode; below?: ReactNode }) {
  return (
    <CenteredPage brand={<CloudBrand />} footer={<CloudFooter />} below={below}>
      {children}
    </CenteredPage>
  );
}
export { SwitchLine } from "@orchester/ui/components/frame";
