declare const __COMMIT_HASH__: string;

declare global {
  namespace App {
    interface PageData {
      seo?: import('$lib/seo').SeoMetadata;
    }
  }
}

export {};
