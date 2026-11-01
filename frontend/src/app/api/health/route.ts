import { NextResponse } from "next/server";

export async function GET() {
  return NextResponse.json(
    {
      status: "ok",
      service: "resource-center-web",
      environment: process.env.RC_ENVIRONMENT ?? "test",
      auth0Configured: Boolean(
        process.env.NEXT_PUBLIC_AUTH0_DOMAIN &&
          process.env.NEXT_PUBLIC_AUTH0_CLIENT_ID &&
          process.env.NEXT_PUBLIC_AUTH0_AUDIENCE
      ),
    },
    { status: 200 }
  );
}
