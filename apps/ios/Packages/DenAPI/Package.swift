// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "DenAPI",
    platforms: [.iOS(.v26), .macOS(.v15)],
    products: [.library(name: "DenAPI", targets: ["DenAPI"])],
    dependencies: [
        .package(url: "https://github.com/apple/swift-http-types", exact: "1.8.0"),
        .package(url: "https://github.com/apple/swift-openapi-generator", exact: "1.13.1"),
        .package(url: "https://github.com/apple/swift-openapi-runtime", exact: "1.12.1"),
        .package(url: "https://github.com/apple/swift-openapi-urlsession", exact: "1.3.1")
    ],
    targets: [
        .target(name: "DenAPI", dependencies: [
            .product(name: "HTTPTypes", package: "swift-http-types"),
            .product(name: "OpenAPIRuntime", package: "swift-openapi-runtime"),
            .product(name: "OpenAPIURLSession", package: "swift-openapi-urlsession")
        ], plugins: [.plugin(name: "OpenAPIGenerator", package: "swift-openapi-generator")])
    ]
)
