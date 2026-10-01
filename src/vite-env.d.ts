/// <reference types="vite/client" />

declare const __TENANT__: {
  id: string;
  productName: string;
  tagline: string;
  /** Rule ids enabled for this tenant, or null for all rules. */
  rules: string[] | null;
};

declare module "virtual:tenant-logo" {
  /** URL of the tenant's logo image, or null when the tenant has none. */
  const logo: string | null;
  export default logo;
}
