declare global {
  const __COMMIT_HASH__: string;

  namespace App {
    interface PageData {
      seo?: import('$lib/seo').SeoMetadata;
    }
  }
}

export {};
