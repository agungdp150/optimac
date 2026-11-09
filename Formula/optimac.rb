class Optimac < Formula
  desc "Read-only macOS filesystem inspector with a responsive terminal UI"
  homepage "https://github.com/agungdp150/optimac"
  head "https://github.com/agungdp150/optimac.git", branch: "main"
  license "MIT"
  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "OptiMac", shell_output("#{bin}/optimac --help")
  end
end
