class Member {
  final String name;
  final bool host;

  Member({
    required this.name,
    required this.host,
  });

  factory Member.fromJson(Map<String, dynamic> json) {
    return Member(
      name: json["name"] ?? "",
      host: json["host"] ?? false,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      "name": name,
      "host": host,
    };
  }
}