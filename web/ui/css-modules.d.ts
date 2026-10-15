/** CSS modules as the app's bundler (Next.js) gives them: class names by key. */
declare module "*.module.css" {
  const classes: Readonly<Record<string, string>>;
  export default classes;
}
