import { NextResponse } from "next/server";
import { auth0Configured } from "@/lib/auth0";

export async function GET() {
  return NextResponse.json(
    {
      status: "ok",
      service: "resource-center-web",
      environment: process.env.RC_ENVIRONMENT ?? "test",
      version: process.env.RC_VERSION ?? "dev",
      auth0Configured,
    },
    { status: 200 }
  );
}
