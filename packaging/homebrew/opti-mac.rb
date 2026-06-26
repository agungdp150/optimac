class OptiMac < Formula
  desc "Safe macOS cleanup and maintenance CLI"
  homepage "https://github.com/luceid/opti-mac"
  url "https://github.com/luceid/opti-mac/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_RELEASE_SHA256"
  license "MIT"

  depends_on "go" => :build

  def install
    system "go", "build", "-ldflags", "-X main.version=#{version}", "-o", bin/"opti-mac", "./cmd/opti-mac"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/opti-mac version")
  end
end
