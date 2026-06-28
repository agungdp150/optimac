class Optimac < Formula
  desc "Safe macOS cleanup and maintenance CLI"
  homepage "https://github.com/agungdp150/optimac"
  url "https://github.com/agungdp150/optimac/archive/refs/tags/v0.1.1.tar.gz"
  sha256 "1a28f93612891e398942f1dd3eb55435a6686e32b09541efa134226d0729909b"
  license "MIT"

  depends_on "go" => :build

  def install
    system "go", "build", "-ldflags", "-X main.version=#{version}", "-o", bin/"optimac", "./cmd/optimac"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/optimac version")
  end
end
