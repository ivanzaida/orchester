import { redirect } from "next/navigation";

/** The server answers `/` with a redirect itself; this is for the admin reached directly (development, tests). */
export default function Root() {
  redirect("/admin");
}
